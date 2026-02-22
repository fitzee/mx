IMPLEMENTATION MODULE Http2Middleware;

  FROM SYSTEM IMPORT ADDRESS;
  FROM Http2ServerTypes IMPORT Request, Response,
                                MiddlewareProc, HandlerProc,
                                MaxMiddleware;
  FROM Log IMPORT Logger, Info;

  TYPE
    LoggerPtr  = POINTER TO Logger;
    LimitPtr   = POINTER TO CARDINAL;

  PROCEDURE ChainInit(VAR c: Chain);
  BEGIN
    c.count := 0;
  END ChainInit;

  PROCEDURE ChainAdd(VAR c: Chain;
                     mw: MiddlewareProc;
                     ctx: ADDRESS): BOOLEAN;
  VAR
    idx: CARDINAL;
  BEGIN
    IF c.count >= MaxMiddleware THEN
      RETURN FALSE
    END;
    idx := c.count;
    c.entries[idx].proc := mw;
    c.entries[idx].ctx := ctx;
    INC(c.count);
    RETURN TRUE
  END ChainAdd;

  PROCEDURE ChainRun(VAR c: Chain;
                     VAR req: Request;
                     VAR resp: Response;
                     handler: HandlerProc;
                     handlerCtx: ADDRESS);
  VAR
    i: CARDINAL;
    ok: BOOLEAN;
  BEGIN
    FOR i := 0 TO c.count - 1 DO
      ok := c.entries[i].proc(req, resp, c.entries[i].ctx);
      IF NOT ok THEN
        RETURN
      END;
    END;
    handler(req, resp, handlerCtx);
  END ChainRun;

  PROCEDURE LoggingMw(VAR req: Request;
                      VAR resp: Response;
                      ctx: ADDRESS): BOOLEAN;
  VAR
    lp: LoggerPtr;
  BEGIN
    lp := LoggerPtr(ctx);
    Info(lp^, req.method);
    RETURN TRUE
  END LoggingMw;

  PROCEDURE SizeLimitMw(VAR req: Request;
                        VAR resp: Response;
                        ctx: ADDRESS): BOOLEAN;
  VAR
    lp: LimitPtr;
  BEGIN
    lp := LimitPtr(ctx);
    IF req.bodyLen > lp^ THEN
      resp.status := 413;
      RETURN FALSE
    END;
    RETURN TRUE
  END SizeLimitMw;

  PROCEDURE GuardMw(VAR req: Request;
                    VAR resp: Response;
                    ctx: ADDRESS): BOOLEAN;
  BEGIN
    (* Guard is a no-op before the handler.
       Status-defaulting (0 → 500) belongs in the connection driver
       after the handler has run. *)
    RETURN TRUE
  END GuardMw;

END Http2Middleware.
