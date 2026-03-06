IMPLEMENTATION MODULE Http2ServerTypes;

  FROM SYSTEM IMPORT ADDRESS;
  FROM ByteBuf IMPORT Buf, Init, Free;

  PROCEDURE InitDefaultOpts(VAR opts: ServerOpts);
  BEGIN
    opts.port := 8443;
    opts.certPath[0] := 0C;
    opts.keyPath[0] := 0C;
    opts.maxConns := 16;
    opts.maxStreams := 32;
    opts.idleTimeoutMs := 30000;
    opts.hsTimeoutMs := 5000;
    opts.drainTimeoutMs := 10000;
  END InitDefaultOpts;

  PROCEDURE InitRequest(VAR req: Request);
  VAR
    i: CARDINAL;
  BEGIN
    req.method[0] := 0C;
    req.path[0] := 0C;
    req.scheme[0] := 0C;
    req.authority[0] := 0C;
    req.numHeaders := 0;
    FOR i := 0 TO MaxReqHeaders - 1 DO
      req.headers[i].name[0] := 0C;
      req.headers[i].nameLen := 0;
      req.headers[i].value[0] := 0C;
      req.headers[i].valLen := 0;
    END;
    Init(req.body, 1024);
    req.bodyLen := 0;
    req.streamId := 0;
    req.connId := 0;
    req.startTick := 0;
    req.remoteAddr[0] := 0C;
    req.connPtr := NIL;
  END InitRequest;

  PROCEDURE InitResponse(VAR resp: Response);
  VAR
    i: CARDINAL;
  BEGIN
    resp.status := 200;
    resp.numHeaders := 0;
    FOR i := 0 TO MaxRespHeaders - 1 DO
      resp.headers[i].name[0] := 0C;
      resp.headers[i].nameLen := 0;
      resp.headers[i].value[0] := 0C;
      resp.headers[i].valLen := 0;
    END;
    Init(resp.body, 4096);
    resp.bodyLen := 0;
  END InitResponse;

  PROCEDURE FreeRequest(VAR req: Request);
  BEGIN
    Free(req.body);
  END FreeRequest;

  PROCEDURE FreeResponse(VAR resp: Response);
  BEGIN
    Free(resp.body);
  END FreeResponse;

END Http2ServerTypes.
