# Auth Architecture

## Layering

```
Application Handlers
        |
  AuthMiddleware       (Http2Server MiddlewareProc)
        |
      Auth             (keyring, verifier, policy, replay)
        |
   AuthBridge          (DEFINITION MODULE FOR "C")
        |
   auth_bridge.c       (OpenSSL: HMAC-SHA256, Ed25519, base64url)
        |
     OpenSSL           (system library, linked via -lssl -lcrypto)
```

## Token Formats

### JWT HS256 (primary, universal)

Standard JWT with three base64url-encoded parts separated by dots:

```
<header>.<payload>.<signature>
```

- **Header**: `{"alg":"HS256","typ":"JWT","kid":"..."}` (kid optional)
- **Payload**: Flat JSON object with standard and custom claims
- **Signature**: HMAC-SHA256(key, "header.payload")

### Ed25519 / PASETO v4.public (optional)

Available when `m2_auth_has_ed25519()` returns 1 (OpenSSL >= 1.1.1).

Format: `v4.public.<base64url(payload || 64-byte-signature)>`

Not yet fully implemented in the M2 signing path. The C bridge provides sign/verify primitives.

## Key Management

The Keyring stores up to 8 keys of mixed types (HS256 symmetric, Ed25519 public). One key is active at a time for signing. Verification tries the key specified by the token's `kid` header claim, falling back to the active key.

Key rotation workflow:
1. Add new key with `KeyringAddHS256`
2. Set new key active with `KeyringSetActive`
3. Keep old key for verification grace period
4. Remove old key with `KeyringRemove`

## State Propagation

Single-threaded design: `AuthMiddleware` stores the last verified Principal in a module-level variable. This is safe because Http2Server processes one request at a time in the event loop. Handlers call `GetPrincipal()` synchronously.

For multi-threaded scenarios, each thread would need its own middleware instance or a thread-local storage pattern.

## Dependencies

```
m2auth
  +-- m2bytes        (ByteBuf for potential body processing)
  +-- m2http2server   (Request/Response types, MiddlewareProc)
  +-- auth_bridge.c   (OpenSSL FFI)
```

The `m2http2server` dependency is needed for the middleware types. Auth.mod itself only depends on AuthBridge (C FFI) and standard library modules (Storage, Strings).

## OpenSSL Compatibility

| OpenSSL Version | HMAC | Ed25519 | Notes |
|-----------------|------|---------|-------|
| 1.1.0+          | HMAC() one-shot | No | Minimum supported |
| 1.1.1+          | HMAC() one-shot | EVP_DigestSign/Verify | Full feature set |
| 3.0+            | EVP_MAC API | EVP_DigestSign/Verify | Deprecated API compat |
| LibreSSL >= 2.7 | HMAC() one-shot | Varies | Tested compatible |

The C bridge detects the OpenSSL version at compile time and selects the appropriate API. The `m2_auth_has_ed25519()` function reports availability at runtime.

## Security Properties

- No `"none"` algorithm accepted
- Constant-time signature comparison (CRYPTO_memcmp)
- Clock skew tolerance for exp/nbf (configurable, default 60s)
- JTI replay cache with time-based eviction
- All buffers are bounded (no unbounded allocation)
- No dynamic dispatch in the verification path
