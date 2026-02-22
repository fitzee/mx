# Auth

Core authentication and authorization module. Provides keyring management, JWT HS256 token signing and verification, Ed25519 PASETO-like tokens (when OpenSSL >= 1.1.1), policy-based authorization, and replay detection.

## Design

All cryptographic operations delegate to `auth_bridge.c` which wraps OpenSSL. The M2 side owns all token construction, parsing, and validation policy. No heap allocation in the hot path -- all buffers are stack-allocated with fixed maximums.

Pointer types (Keyring, Verifier, Policy, ReplayCache) are allocated via `Storage.ALLOCATE/DEALLOCATE`.

## Types

### Status

```modula2
Status = (OK, Invalid, OutOfMemory, VerifyFailed,
          Expired, NotYetValid, BadSignature, Unsupported, Denied)
```

All procedures return Status. OK indicates success. Use `StatusToStr` for human-readable output.

### TokenKind

```modula2
TokenKind = (PasetoV4Public, JwtHS256, JwtES256)
```

Select the signing algorithm. JwtHS256 is universally available. PasetoV4Public requires Ed25519 support (check `m2_auth_has_ed25519()`). JwtES256 is reserved for future use.

### Principal

```modula2
Principal = RECORD
  subject, issuer, audience: ...;
  scopes: ARRAY[0..15] OF ARRAY[0..63] OF CHAR;
  scopeCount: CARDINAL;
  claims: Claims;
  expUnix, nbfUnix, iatUnix: LONGINT;
  kid, jti: ...;
END
```

Populated by `VerifyBearerToken`. Contains all decoded token claims. Known JWT keys (sub, iss, aud, exp, nbf, iat, jti, scope, kid) are mapped to named fields. Unknown keys become custom Claims.

### Claim

```modula2
Claim = RECORD
  key: ARRAY[0..31] OF CHAR;
  vtype: ClaimType;  (* Str, Int, Bool *)
  s: ARRAY[0..255] OF CHAR;
  i: LONGINT;
  b: BOOLEAN;
END
```

Custom claim with tagged value. Up to 32 custom claims per token.

## Keyring

Manages signing and verification keys. Up to 8 keys, one active at a time.

```modula2
PROCEDURE KeyringCreate(VAR kr: Keyring): Status;
PROCEDURE KeyringDestroy(VAR kr: Keyring): Status;
PROCEDURE KeyringAddHS256(kr: Keyring; kid: ARRAY OF CHAR; VAR key: SymKey): Status;
PROCEDURE KeyringAddEd25519Public(kr: Keyring; kid: ARRAY OF CHAR; VAR key: PublicKey): Status;
PROCEDURE KeyringRemove(kr: Keyring; kid: ARRAY OF CHAR): Status;
PROCEDURE KeyringSetActive(kr: Keyring; kid: ARRAY OF CHAR): Status;
PROCEDURE KeyringList(kr: Keyring; VAR kids: ARRAY OF KeyId; VAR count: CARDINAL): Status;
```

The first added key becomes active by default. Use `KeyringSetActive` to rotate.

## Verifier

Configurable token verifier. Holds a reference to a Keyring.

```modula2
PROCEDURE VerifierCreate(VAR v: Verifier; kr: Keyring): Status;
PROCEDURE VerifierDestroy(VAR v: Verifier): Status;
PROCEDURE VerifierSetClockSkewSeconds(v: Verifier; secs: CARDINAL): Status;
PROCEDURE VerifierSetAudience(v: Verifier; aud: ARRAY OF CHAR): Status;
PROCEDURE VerifierSetIssuer(v: Verifier; iss: ARRAY OF CHAR): Status;
PROCEDURE VerifyBearerToken(v: Verifier; token: ARRAY OF CHAR; VAR principal: Principal): Status;
```

Default clock skew is 60 seconds. Set audience/issuer to empty string to disable validation.

## Signing

```modula2
PROCEDURE SignToken(kr: Keyring; kind: TokenKind; kid: ARRAY OF CHAR;
                    VAR principal: Principal;
                    VAR token: ARRAY OF CHAR; VAR tokenLen: CARDINAL): Status;
```

Signs a Principal into a token string. The `kid` selects which key from the keyring to use (pass empty string to use the active key).

## JWT HS256 Verification Flow

1. Split token at `.` into header, payload, signature (exactly 3 parts)
2. Base64url-decode header; parse for `alg`, `kid`, `typ`; reject `"none"` algorithm
3. Look up key in keyring by `kid` (or use active key)
4. HMAC-SHA256(key, "header.payload") with constant-time signature comparison
5. Base64url-decode payload; parse JSON into Principal
6. Validate `exp` (with clock skew), `nbf` (with clock skew), `iss`, `aud`

## Policy

Deny-by-default authorization. Rules are OR'd -- any matching rule allows access.

```modula2
PROCEDURE PolicyCreate(VAR pol: Policy): Status;
PROCEDURE PolicyDestroy(VAR pol: Policy): Status;
PROCEDURE PolicyAllowScope(pol: Policy; scope: ARRAY OF CHAR): Status;
PROCEDURE PolicyAllowClaimEquals(pol: Policy; key, value: ARRAY OF CHAR): Status;
PROCEDURE Authorize(pol: Policy; VAR principal: Principal): Status;
```

## Replay Cache

Ring-buffer JTI cache with time-based eviction. Capacity: 256 entries.

```modula2
PROCEDURE ReplayCacheCreate(VAR rc: ReplayCache): Status;
PROCEDURE ReplayCacheDestroy(VAR rc: ReplayCache): Status;
PROCEDURE ReplayCacheSeenOrAdd(rc: ReplayCache; jti: ARRAY OF CHAR; expUnix: LONGINT): Status;
```

Returns OK if the JTI is new, VerifyFailed if already seen.

## Threat Model

- **Algorithm confusion**: Only HS256 and Ed25519 accepted; `"none"` algorithm explicitly rejected
- **Timing attacks**: Signature comparison uses OpenSSL's `CRYPTO_memcmp` (constant-time)
- **Replay attacks**: Optional JTI-based replay cache with configurable capacity
- **Clock skew**: Configurable tolerance for `exp`/`nbf` validation (default 60s)
- **Key rotation**: Keyring supports multiple keys with explicit activation
- **Token size**: Maximum 2048 bytes; fits within Http2ServerTypes.MaxReqValueLen (255) for typical JWT HS256 with minimal claims

## Known Limits

- Maximum token length: 2048 bytes
- Maximum custom claims: 32
- Maximum keys in keyring: 8
- Maximum scopes: 16
- Maximum replay cache entries: 256
- JWT header `Authorization: Bearer <token>` must fit in MaxReqValueLen (255 bytes)
- JSON parser is flat-object only (no nested objects or arrays)
