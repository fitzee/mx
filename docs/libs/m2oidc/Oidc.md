# m2oidc

OIDC (OpenID Connect) library for Modula-2. Provides RS256 JWT
verification with JWKS key management and Keycloak-style claim
extraction. Designed for server-side token validation against external
identity providers (Keycloak, Okta, Auth0).

## Modules

| Module | Purpose |
|--------|---------|
| `Oidc.def/mod` | OIDC provider, discovery parsing, JWT RS256 verification, claim extraction. |
| `Jwks.def/mod` | JWKS key set management — parse JWKS JSON, store RSA keys by kid, RS256 signature verification. Thread-safe (mutex-protected). |
| `OidcBridge.def` + `oidc_bridge.c` | Thin C FFI to OpenSSL for RSA key construction and verification. |

## Dependencies

| Library | Used for |
|---------|----------|
| `m2auth` | `m2_auth_b64url_decode`, `m2_auth_get_unix_time` |
| `m2json` | SAX JSON parser for JWKS and JWT payload parsing |
| `m2pthreads` | Mutex for thread-safe KeySet access |
| OpenSSL | RSA public key construction (EVP_PKEY) and signature verification |

## m2.toml

```toml
manifest_version = 1
name = m2oidc
version = 0.1.0
edition = pim4
entry = src/Oidc.mod
includes = src
[deps]
m2auth = "path:../m2auth"
m2json = "path:../m2json"
m2pthreads = "path:../m2pthreads"
[cc]
extra-c = src/oidc_bridge.c
cflags = -I/opt/homebrew/opt/openssl@3/include
ldflags = -L/opt/homebrew/opt/openssl@3/lib
libs = -lssl -lcrypto
```

---

## Oidc Module

### Status

```modula2
Status = (OcOk, OcInvalid, OcBadSignature, OcExpired, OcNotYetValid,
          OcBadIssuer, OcBadAudience, OcNoKid, OcKeyNotFound,
          OcParseFailed, OcUnsupportedAlg, OcOutOfMemory);
```

### OidcClaims

```modula2
OidcClaims = RECORD
  subject:    ARRAY [0..127] OF CHAR;
  issuer:     ARRAY [0..255] OF CHAR;
  audience:   ARRAY [0..127] OF CHAR;
  username:   ARRAY [0..127] OF CHAR;   (* preferred_username *)
  email:      ARRAY [0..255] OF CHAR;
  azp:        ARRAY [0..127] OF CHAR;   (* authorized party *)
  expUnix:    LONGINT;
  nbfUnix:    LONGINT;
  iatUnix:    LONGINT;
  roles:      ARRAY [0..15] OF ARRAY [0..63] OF CHAR;
  roleCount:  CARDINAL;
  groups:     ARRAY [0..15] OF ARRAY [0..127] OF CHAR;
  groupCount: CARDINAL;
END;
```

Populated by `VerifyToken`. Standard JWT claims (sub, iss, aud, exp,
nbf, iat) plus Keycloak extensions (preferred_username, email, azp,
realm_access.roles, groups).

### CreateProvider / DestroyProvider

```modula2
PROCEDURE CreateProvider(VAR prov: Provider;
                         VAR issuer, clientId: ARRAY OF CHAR;
                         clockSkewSecs: CARDINAL;
                         keySet: ADDRESS): Status;
PROCEDURE DestroyProvider(VAR prov: Provider): Status;
```

Create a provider bound to a specific issuer URL and expected audience
(client ID). `keySet` is a `Jwks.KeySet` handle. `clockSkewSecs` is
the tolerance for exp/nbf validation (typically 30s).

DestroyProvider frees the provider but does NOT destroy the KeySet —
the caller owns that lifecycle.

### ParseDiscovery

```modula2
PROCEDURE ParseDiscovery(json: ADDRESS; jsonLen: CARDINAL;
                         VAR issuerOut, jwksUriOut: ARRAY OF CHAR): Status;
```

Parse an OIDC discovery document (the JSON from
`/.well-known/openid-configuration`). Extracts the `issuer` and
`jwks_uri` fields. Standalone — does not require a Provider.

### VerifyToken

```modula2
PROCEDURE VerifyToken(prov: Provider;
                      VAR token: ARRAY OF CHAR;
                      VAR claims: OidcClaims): Status;
```

Full RS256 JWT verification and claim extraction:

1. Split token at dots into header, payload, signature.
2. Base64url-decode header, parse for `alg` (must be RS256) and `kid`.
3. Look up kid in the provider's KeySet via `Jwks.FindKey`.
4. Verify signature via `Jwks.VerifyRS256` — signing input is the raw
   base64url text `header.payload` (NOT decoded bytes).
5. Base64url-decode payload, parse claims:
   - Standard: sub, iss, aud (string or array), exp, nbf, iat, azp.
   - Keycloak: `realm_access` object → `roles` array, `groups` array.
   - Username: `preferred_username`. Email: `email`.
   - SPECTRA: `role` (single string, added to roles array).
6. Validate: issuer matches provider, audience matches clientId,
   exp/nbf with clock skew tolerance.

Returns `OcOk` on success. Specific error codes for each failure mode
allow callers to return appropriate HTTP error messages.

### PeekAlg

```modula2
PROCEDURE PeekAlg(VAR token: ARRAY OF CHAR;
                  VAR algOut: ARRAY OF CHAR): Status;
```

Peek at the `alg` field in a JWT header without full verification.
Used by `Dispatch.mod` to route between the HS256 and RS256
verification paths.

### GetKeySet

```modula2
PROCEDURE GetKeySet(prov: Provider): ADDRESS;
```

Returns the `Jwks.KeySet` handle associated with a provider. Used for
JWKS refresh — fetch new JWKS JSON and call `Jwks.ParseJson` on the
existing KeySet to atomically replace all keys.

---

## Jwks Module

Thread-safe JWKS key set management. All operations acquire an internal
mutex.

### Status

```modula2
Status = (JkOk, JkInvalid, JkNoSuchKid, JkFull,
          JkParseFailed, JkOutOfMemory);
```

### Constants

```modula2
MaxKeys   = 16;   (* maximum RSA keys in one set *)
MaxKidLen = 63;   (* maximum kid string length *)
```

### Create / Destroy

```modula2
PROCEDURE Create(VAR ks: KeySet): Status;
PROCEDURE Destroy(VAR ks: KeySet): Status;
```

Create an empty key set. Destroy frees all RSA key handles (via
OpenSSL) and the key set itself.

### ParseJson

```modula2
PROCEDURE ParseJson(ks: KeySet;
                    json: ADDRESS; jsonLen: CARDINAL): Status;
```

Parse a JWKS JSON document and replace all keys in the set. Navigates
to the `"keys"` array, extracts each JWK object with `kty=RSA` and
`alg=RS256` (or absent alg), base64url-decodes `n` and `e`, constructs
RSA public key handles via `OidcBridge.m2_oidc_rsa_from_ne`.

Clears existing keys before adding new ones (under lock). Keys without
a `kid` field are skipped. Non-RSA and non-RS256 keys are skipped.

Safe to call repeatedly for key refresh — existing keys are freed and
replaced atomically (under mutex).

### FindKey

```modula2
PROCEDURE FindKey(ks: KeySet;
                  VAR kid: ARRAY OF CHAR): Status;
```

Check whether an RSA key with the given `kid` exists. Returns `JkOk`
if found, `JkNoSuchKid` if not. Existence check only — does not
return the key handle.

### Count

```modula2
PROCEDURE Count(ks: KeySet): CARDINAL;
```

Return the number of keys currently loaded.

### VerifyRS256

```modula2
PROCEDURE VerifyRS256(ks: KeySet;
                      VAR kid: ARRAY OF CHAR;
                      msg: ADDRESS; msgLen: CARDINAL;
                      sig: ADDRESS; sigLen: CARDINAL): Status;
```

Verify an RS256 (RSASSA-PKCS1-v1_5 + SHA-256) signature using the key
identified by `kid`. Does its own kid lookup internally.

- `msg/msgLen`: the signing input (raw base64url text `header.payload`).
- `sig/sigLen`: the raw decoded signature bytes.
- Returns `JkOk` if valid, `JkNoSuchKid` if kid not found, `JkInvalid`
  if signature verification fails.

---

## OidcBridge (C FFI)

Three C functions wrapping OpenSSL. Uses `OSSL_PARAM_BLD` +
`EVP_PKEY_fromdata` on OpenSSL >= 3.0, falls back to `RSA_new` +
`RSA_set0_key` on older versions.

```modula2
DEFINITION MODULE FOR "C" OidcBridge;
  FROM SYSTEM IMPORT ADDRESS;

  (* Construct RSA public key from raw big-endian n + e bytes.
     Returns opaque EVP_PKEY* handle, or NIL on failure. *)
  PROCEDURE m2_oidc_rsa_from_ne(n: ADDRESS; nLen: INTEGER;
                                 e: ADDRESS; eLen: INTEGER): ADDRESS;

  (* Verify RS256 signature. Returns 0 if valid, -1 if invalid. *)
  PROCEDURE m2_oidc_rsa_verify(key: ADDRESS;
                                msg: ADDRESS; msgLen: INTEGER;
                                sig: ADDRESS; sigLen: INTEGER): INTEGER;

  (* Free an RSA key handle (EVP_PKEY_free). *)
  PROCEDURE m2_oidc_rsa_free(key: ADDRESS);
END OidcBridge.
```

---

## Usage

### Typical server integration

The caller is responsible for fetching the OIDC discovery and JWKS
JSON documents. SPECTRA uses `OidcFetch.mod` which wraps m2http's
`HTTPClient.Get` with a temporary `EventLoop` for a blocking HTTPS
request. Any HTTP client will work — the library only needs the raw
JSON bytes.

```
Startup:
  1. Jwks.Create(keySet)
  2. HTTP GET issuer/.well-known/openid-configuration
  3. Oidc.ParseDiscovery(json, len, issuerOut, jwksUriOut)
  4. HTTP GET jwksUri
  5. Jwks.ParseJson(keySet, jwksJson, jwksLen)
  6. Oidc.CreateProvider(prov, issuer, clientId, 30, keySet)
  7. Register JWKS refresh timer

Per request:
  1. Oidc.PeekAlg(token, algBuf)
  2. If algBuf = "RS256":
       Oidc.VerifyToken(prov, token, claims) → map roles
  3. Else:
       Auth.VerifyBearerToken (HS256 path)

JWKS refresh (timer callback):
  1. Fetch JWKS JSON
  2. ks := Oidc.GetKeySet(prov)
  3. Jwks.ParseJson(ks, json, len)  (* atomic replace under lock *)

Shutdown:
  1. Oidc.DestroyProvider(prov)
  2. Jwks.Destroy(keySet)
```

### Keycloak role mapping

Keycloak tokens carry roles in `realm_access.roles`:

```json
{
  "realm_access": {
    "roles": ["admin", "uma_authorization"]
  },
  "preferred_username": "matt",
  "email": "matt@example.com"
}
```

After `VerifyToken`, scan `claims.roles[0..roleCount-1]` for
application-specific roles (e.g. admin, operator, annotator, viewer).

## Known Limits

- Maximum keys per KeySet: 16
- Maximum kid length: 63 characters
- Maximum RSA modulus: 4096-bit (512 bytes decoded n)
- Maximum token size: limited by caller's buffer (VAR token: ARRAY OF CHAR)
- Maximum payload: 8192 bytes decoded
- Maximum roles: 16
- Maximum groups: 16
- Only RS256 algorithm supported (no RS384, RS512, ES256, EdDSA)
- JWKS must contain `"keys"` array at the top level
- Each JWK must have a `kid` field to be usable

## Known mx Issue

Assignment to a `VAR key: ADDRESS` out parameter generates incorrect C
code (`memcpy` instead of direct pointer assignment). The `FindKey`
procedure was simplified to return only a status (no key output) as a
workaround. `VerifyRS256` does its own internal kid lookup and does not
expose the raw key handle.

See `mx-pitfalls.md` item 9 for details.
