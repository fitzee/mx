IMPLEMENTATION MODULE Http2Router;

  FROM SYSTEM IMPORT ADDRESS;
  FROM Http2ServerTypes IMPORT Request, Response, HandlerProc,
                                MaxRoutes, MaxMethodLen, MaxPathLen;

  PROCEDURE CopyStr(src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
  VAR
    i: CARDINAL;
    lim: CARDINAL;
  BEGIN
    IF HIGH(src) < HIGH(dst) THEN
      lim := HIGH(src)
    ELSE
      lim := HIGH(dst)
    END;
    i := 0;
    WHILE (i <= lim) AND (src[i] # 0C) DO
      dst[i] := src[i];
      INC(i);
    END;
    IF i <= HIGH(dst) THEN
      dst[i] := 0C;
    END;
  END CopyStr;

  PROCEDURE StrEq(a, b: ARRAY OF CHAR): BOOLEAN;
  VAR
    i: CARDINAL;
    lim: CARDINAL;
  BEGIN
    IF HIGH(a) < HIGH(b) THEN
      lim := HIGH(a)
    ELSE
      lim := HIGH(b)
    END;
    i := 0;
    WHILE (i <= lim) AND (a[i] # 0C) AND (b[i] # 0C) DO
      IF a[i] # b[i] THEN
        RETURN FALSE
      END;
      INC(i);
    END;
    (* Both must be at end-of-string *)
    IF (i <= HIGH(a)) AND (a[i] # 0C) THEN RETURN FALSE END;
    IF (i <= HIGH(b)) AND (b[i] # 0C) THEN RETURN FALSE END;
    RETURN TRUE
  END StrEq;

  PROCEDURE Default404(VAR req: Request; VAR resp: Response;
                        ctx: ADDRESS);
  BEGIN
    resp.status := 404;
  END Default404;

  PROCEDURE RouterInit(VAR r: Router);
  BEGIN
    r.count := 0;
  END RouterInit;

  PROCEDURE AddRoute(VAR r: Router;
                     method, path: ARRAY OF CHAR;
                     handler: HandlerProc;
                     ctx: ADDRESS): BOOLEAN;
  VAR
    idx: CARDINAL;
  BEGIN
    IF r.count >= MaxRoutes THEN
      RETURN FALSE
    END;
    idx := r.count;
    CopyStr(method, r.routes[idx].method);
    CopyStr(path, r.routes[idx].path);
    r.routes[idx].handler := handler;
    r.routes[idx].ctx := ctx;
    INC(r.count);
    RETURN TRUE
  END AddRoute;

  PROCEDURE Dispatch(VAR r: Router;
                     VAR req: Request;
                     VAR resp: Response);
  VAR
    i: CARDINAL;
  BEGIN
    IF r.count > 0 THEN
      FOR i := 0 TO r.count - 1 DO
        IF StrEq(req.method, r.routes[i].method) AND
           StrEq(req.path, r.routes[i].path) THEN
          r.routes[i].handler(req, resp, r.routes[i].ctx);
          RETURN
        END;
      END;
    END;
    (* No route matched — return 404 *)
    Default404(req, resp, NIL);
  END Dispatch;

END Http2Router.
