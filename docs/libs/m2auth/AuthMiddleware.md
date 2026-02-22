# AuthMiddleware

HTTP/2 server middleware and RPC auth guard for m2auth. Integrates token verification and policy authorization into the Http2Server request pipeline.

## Design

Single-threaded module. A module-level variable holds the last verified Principal. After `AuthMw` succeeds, handlers call `GetPrincipal()` to access the authenticated identity.

## Configuration

```modula2
PROCEDURE Configure(v: Verifier; pol: Policy);
```

Must be called before adding `AuthMw` to the server. Pass NIL for `pol` to skip authorization (authentication only).

## HTTP/2 Middleware

```modula2
PROCEDURE AuthMw(VAR req: Request; VAR resp: Response; ctx: ADDRESS): BOOLEAN;
```

An `Http2ServerTypes.MiddlewareProc`. Extracts the Bearer token from the `Authorization` header, verifies it via the configured Verifier, and optionally authorizes via the configured Policy.

Returns TRUE to continue the middleware chain (auth OK). Returns FALSE to short-circuit (sets 401 or 403 status on the response).

### Integration

```modula2
Configure(myVerifier, myPolicy);
AddMiddleware(server, AuthMw, NIL);
```

## Principal Access

```modula2
PROCEDURE GetPrincipal(VAR p: Principal): BOOLEAN;
```

Returns TRUE and populates `p` if the last `AuthMw` call succeeded. Returns FALSE if no principal is available.

```modula2
PROCEDURE MyHandler(VAR req: Request; VAR resp: Response; ctx: ADDRESS);
VAR p: Principal;
BEGIN
  IF GetPrincipal(p) THEN
    (* p.subject, p.scopes, p.claims available *)
  END
END MyHandler;
```

## Audit

```modula2
TYPE AuditProc = PROCEDURE(VAR AuditEvent, ADDRESS);
PROCEDURE SetAuditCallback(proc: AuditProc; ctx: ADDRESS);
```

Optional callback invoked on every auth decision (success, failure, or denial). The AuditEvent contains the decision kind, subject, HTTP method, path, auth status, and remote address.

## RPC Auth Guard

For m2rpc services running over Http2Server, the middleware handles auth before RPC dispatch. No special RPC-level guard is needed.

For standalone m2rpc, `RpcAuthGuard` wraps an existing handler:

```modula2
PROCEDURE SetRpcGuardHandler(innerHandler: ...);
PROCEDURE RpcAuthGuard(ctx, reqId, methodPtr, methodLen, body, outBody, errCode, ok);
```

The guard checks that a principal was previously authenticated (by the middleware or by the caller). If authorized, it delegates to the inner handler.
