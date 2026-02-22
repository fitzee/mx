# Http2Router

Method + path router for HTTP/2 requests. Dispatches to registered handlers using exact string matching with a linear scan and a built-in default 404 handler.

## Why Linear Scan?

A typical HTTP/2 server has fewer than 64 routes. For this scale, a linear scan over a flat array is simpler and faster than a trie or hash map: no heap allocation, no hash collisions, cache-friendly memory layout, and O(N) lookup where N is small. The `MaxRoutes` constant (64) is the hard cap.

If your service needs hundreds of routes, consider grouping them by prefix in application code and dispatching to sub-routers.

## Design

- **Exact match**: both `method` and `path` must match exactly (case-sensitive string comparison). No wildcards, no path parameters, no regex.
- **First match wins**: routes are checked in registration order. If two routes have the same (method, path), the first one registered wins.
- **Default 404**: if no route matches, the router writes `resp.status := 404` with an empty body. No handler is called.

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxRoutes` | 64 | Maximum registered routes (from `Http2ServerTypes`) |
| `MaxMethodLen` | 15 | Maximum HTTP method length (from `Http2ServerTypes`) |
| `MaxPathLen` | 511 | Maximum path length (from `Http2ServerTypes`) |

## Types

### Route

```modula2
TYPE Route = RECORD
  method:  ARRAY [0..MaxMethodLen] OF CHAR;
  path:    ARRAY [0..MaxPathLen] OF CHAR;
  handler: HandlerProc;
  ctx:     ADDRESS;
END;
```

A single route entry: the (method, path) pair, handler procedure, and handler context pointer.

### Router

```modula2
TYPE Router = RECORD
  routes: ARRAY [0..MaxRoutes-1] OF Route;
  count:  CARDINAL;
END;
```

The route table. `count` tracks the number of registered routes.

## Procedures

### RouterInit

```modula2
PROCEDURE RouterInit(VAR r: Router);
```

Initialise the router to empty (`count := 0`).

### AddRoute

```modula2
PROCEDURE AddRoute(VAR r: Router; method, path: ARRAY OF CHAR;
                   handler: HandlerProc; ctx: ADDRESS): BOOLEAN;
```

Register a handler for an exact (method, path) match.

- `r` -- the router
- `method` -- HTTP method (e.g. `"GET"`, `"POST"`, `"DELETE"`)
- `path` -- request path (e.g. `"/api/users"`, `"/health"`)
- `handler` -- procedure to call when this route matches
- `ctx` -- opaque context pointer passed to the handler on every dispatch

Returns `TRUE` on success, `FALSE` if the route table is full (`MaxRoutes` reached).

### Dispatch

```modula2
PROCEDURE Dispatch(VAR r: Router; VAR req: Request; VAR resp: Response);
```

Dispatch a request to the matching handler.

1. Scans `routes[0..count-1]` comparing `req.method` and `req.path`
2. On match: calls `handler(req, resp, ctx)` for the matching route
3. No match: sets `resp.status := 404` (default 404 handler)

## Example

```modula2
MODULE RouterDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM ByteBuf IMPORT AppendStr;
FROM Http2ServerTypes IMPORT Request, Response, HandlerProc;
FROM Http2Router IMPORT Router, RouterInit, AddRoute, Dispatch;

PROCEDURE HelloHandler(VAR req: Request;
                       VAR resp: Response;
                       ctx: ADDRESS);
BEGIN
  resp.status := 200;
  AppendStr(resp.body, "Hello, world!");
  resp.bodyLen := 13
END HelloHandler;

PROCEDURE EchoHandler(VAR req: Request;
                      VAR resp: Response;
                      ctx: ADDRESS);
BEGIN
  resp.status := 200;
  AppendView(resp.body, AsView(req.body));
  resp.bodyLen := req.bodyLen
END EchoHandler;

PROCEDURE HealthHandler(VAR req: Request;
                        VAR resp: Response;
                        ctx: ADDRESS);
BEGIN
  resp.status := 200;
  AppendStr(resp.body, "ok");
  resp.bodyLen := 2
END HealthHandler;

VAR r: Router;

BEGIN
  RouterInit(r);
  AddRoute(r, "GET", "/hello", HelloHandler, NIL);
  AddRoute(r, "POST", "/echo", EchoHandler, NIL);
  AddRoute(r, "GET", "/health", HealthHandler, NIL);

  (* Dispatch fills resp based on req.method + req.path *)
  Dispatch(r, req, resp)
END RouterDemo.
```
