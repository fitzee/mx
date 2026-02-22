# Http2Middleware

Pre-handler middleware chain for HTTP/2 requests. Each middleware runs in insertion order before the matched handler. If any middleware returns `FALSE`, subsequent middleware and the handler are skipped.

## Why Middleware?

Cross-cutting concerns like logging, request size limits, and error recovery should not be duplicated in every handler. The middleware chain runs a sequence of `MiddlewareProc` callbacks before the handler, giving each one a chance to inspect or modify the request/response, or short-circuit the chain entirely.

## Execution Order

```
Request ──▸ Mw[0] ──TRUE──▸ Mw[1] ──TRUE──▸ ... ──TRUE──▸ Handler
                │                │
             FALSE            FALSE
                │                │
                ▼                ▼
           (response         (response
            already           already
            written)          written)
```

`ChainRun` iterates entries `0..count-1`. Each `MiddlewareProc` receives the request, response, and its registered context pointer. If it returns `TRUE`, the next middleware runs. If it returns `FALSE`, the chain stops and the handler is never called -- the middleware must have written a complete response (status code, headers, and/or body).

After all middleware returns `TRUE`, the matched `HandlerProc` is called.

## Types

### MwEntry

```modula2
TYPE MwEntry = RECORD
  proc: MiddlewareProc;
  ctx:  ADDRESS;
END;
```

A single middleware registration: the callback procedure and its context pointer.

### Chain

```modula2
TYPE Chain = RECORD
  entries: ARRAY [0..MaxMiddleware-1] OF MwEntry;
  count:   CARDINAL;
END;
```

The middleware chain. Holds up to `MaxMiddleware` (8) entries. Entries run in insertion order.

## Procedures

### ChainInit

```modula2
PROCEDURE ChainInit(VAR c: Chain);
```

Initialise the chain to empty (`count := 0`).

### ChainAdd

```modula2
PROCEDURE ChainAdd(VAR c: Chain; mw: MiddlewareProc;
                   ctx: ADDRESS): BOOLEAN;
```

Append middleware to the chain.

- `mw` -- middleware procedure
- `ctx` -- opaque context pointer passed to `mw` on every request

Returns `TRUE` on success, `FALSE` if the chain is full (`MaxMiddleware` reached).

Middleware runs in insertion order, so add logging before size limits if you want rejected requests to still be logged.

### ChainRun

```modula2
PROCEDURE ChainRun(VAR c: Chain; VAR req: Request; VAR resp: Response;
                   handler: HandlerProc; handlerCtx: ADDRESS);
```

Execute the middleware chain, then call the handler if all middleware returned `TRUE`.

- `c` -- the middleware chain
- `req` / `resp` -- request and response (passed through to each middleware and the handler)
- `handler` -- the matched route handler
- `handlerCtx` -- the handler's registered context pointer

## Built-in Middleware

### LoggingMw

```modula2
PROCEDURE LoggingMw(VAR req: Request; VAR resp: Response;
                    ctx: ADDRESS): BOOLEAN;
```

Logs the request method and path at INFO level. Always returns `TRUE` (never short-circuits).

**ctx**: must point to a `Log.Logger` value. Example:

```modula2
VAR logger: Logger;
LogInit(logger);
ChainAdd(chain, LoggingMw, ADR(logger));
```

### SizeLimitMw

```modula2
PROCEDURE SizeLimitMw(VAR req: Request; VAR resp: Response;
                      ctx: ADDRESS): BOOLEAN;
```

Rejects requests whose body exceeds a size limit. If `req.bodyLen` exceeds the limit, sets `resp.status := 413` and returns `FALSE` (short-circuits the chain).

**ctx**: must point to a `CARDINAL` containing the maximum body size in bytes. Example:

```modula2
VAR limit: CARDINAL;
limit := 1048576;   (* 1 MB *)
ChainAdd(chain, SizeLimitMw, ADR(limit));
```

### GuardMw

```modula2
PROCEDURE GuardMw(VAR req: Request; VAR resp: Response;
                  ctx: ADDRESS): BOOLEAN;
```

Error guard middleware. Checks if the handler left `resp.status = 0` (indicating an unhandled error) and sets it to 500. Always returns `TRUE` -- it does not short-circuit, but rather runs as a post-check.

**ctx**: ignored (pass `NIL`).

**Note**: Place `GuardMw` as the last middleware in the chain so it runs immediately before the handler and can inspect the response status afterward.

## Example

```modula2
VAR
  chain: Chain;
  logger: Logger;
  limit: CARDINAL;

ChainInit(chain);

(* 1. Log every request *)
LogInit(logger);
ChainAdd(chain, LoggingMw, ADR(logger));

(* 2. Reject oversized bodies *)
limit := 2097152;   (* 2 MB *)
ChainAdd(chain, SizeLimitMw, ADR(limit));

(* 3. Catch handler errors *)
ChainAdd(chain, GuardMw, NIL);

(* Run the chain for a request *)
ChainRun(chain, req, resp, MyHandler, NIL);
```

### Writing Custom Middleware

```modula2
PROCEDURE AuthMw(VAR req: Request; VAR resp: Response;
                 ctx: ADDRESS): BOOLEAN;
VAR
  i: CARDINAL;
  found: BOOLEAN;
BEGIN
  found := FALSE;
  FOR i := 0 TO req.numHeaders - 1 DO
    IF req.headers[i].name = "authorization" THEN
      found := TRUE
    END
  END;
  IF NOT found THEN
    resp.status := 401;
    RETURN FALSE        (* short-circuit: no handler call *)
  END;
  RETURN TRUE           (* continue chain *)
END AuthMw;
```
