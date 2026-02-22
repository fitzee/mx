IMPLEMENTATION MODULE Http2ServerLog;

  FROM Log IMPORT Logger, Field, Sink, KVStr, KVInt,
                   Init, SetCategory, MakeConsoleSink, AddSink,
                   LogKV, INFO, DEBUG;
  FROM Http2ServerTypes IMPORT Request, Response;

  PROCEDURE LogInit(VAR lg: Logger);
  VAR
    s: Sink;
    ok: BOOLEAN;
  BEGIN
    Init(lg);
    SetCategory(lg, "h2server");
    MakeConsoleSink(s);
    ok := AddSink(lg, s);
  END LogInit;

  PROCEDURE LogRequest(VAR lg: Logger;
                       VAR req: Request;
                       VAR resp: Response;
                       durationTicks: INTEGER);
  VAR
    fields: ARRAY [0..5] OF Field;
  BEGIN
    KVInt("conn", VAL(INTEGER, req.connId), fields[0]);
    KVInt("stream", VAL(INTEGER, req.streamId), fields[1]);
    KVStr("method", req.method, fields[2]);
    KVStr("path", req.path, fields[3]);
    KVInt("status", VAL(INTEGER, resp.status), fields[4]);
    KVInt("dur", durationTicks, fields[5]);
    LogKV(lg, INFO, "request", fields, 6);
  END LogRequest;

  PROCEDURE LogProtocol(VAR lg: Logger;
                        connId: CARDINAL;
                        event: ARRAY OF CHAR;
                        detail: ARRAY OF CHAR);
  VAR
    fields: ARRAY [0..2] OF Field;
  BEGIN
    KVInt("conn", VAL(INTEGER, connId), fields[0]);
    KVStr("event", event, fields[1]);
    KVStr("detail", detail, fields[2]);
    LogKV(lg, DEBUG, "protocol", fields, 3);
  END LogProtocol;

  PROCEDURE LogConn(VAR lg: Logger;
                    connId: CARDINAL;
                    event: ARRAY OF CHAR);
  VAR
    fields: ARRAY [0..1] OF Field;
  BEGIN
    KVInt("conn", VAL(INTEGER, connId), fields[0]);
    KVStr("event", event, fields[1]);
    LogKV(lg, INFO, "connection", fields, 2);
  END LogConn;

END Http2ServerLog.
