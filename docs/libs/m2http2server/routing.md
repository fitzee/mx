# Routing and Middleware

## Route Registration

Routes are registered before `Start` via `AddRoute`:

```modula2
AddRoute(srv, "GET", "/hello", HelloHandler, NIL);
AddRoute(srv, "POST", "/echo", EchoHandler, ADR(echoCtx));
```

- **Exact match** on (method, path)
- Linear scan, first match wins
- Max 64 routes
- Default 404 if no match

## Handler Signature

```modula2
PROCEDURE MyHandler(VAR req: Request; VAR resp: Response; ctx: ADDRESS);
```

- `req` contains method, path, headers, body
- `resp` is pre-initialised with status 200 and empty body
- `ctx` is the ADDRESS passed during `AddRoute`

Set `resp.status` and append body to `resp.body` via ByteBuf.

## Middleware Chain

Middleware runs in insertion order before the handler:

```modula2
AddMiddleware(srv, SizeLimitMw, ADR(maxSize));
AddMiddleware(srv, LoggingMw, ADR(logger));
```

Each middleware returns BOOLEAN:
- TRUE → continue to next middleware / handler
- FALSE → short-circuit (middleware must have written the response)

## Built-in Middleware

| Name | Purpose | ctx parameter |
|---|---|---|
| `LoggingMw` | Log request method at INFO | `POINTER TO Logger` |
| `SizeLimitMw` | Reject bodies > limit (413) | `POINTER TO CARDINAL` |
| `GuardMw` | No-op pass-through | NIL |

## Dispatch Flow

```
Request → Middleware[0] → Middleware[1] → ... → Handler
                ↓ FALSE
          Response (short-circuited)
```
