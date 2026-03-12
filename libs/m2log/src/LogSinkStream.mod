IMPLEMENTATION MODULE LogSinkStream;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Log IMPORT Sink, Record, Level, Format, LineBuf, MaxLine;
FROM Stream IMPORT Stream, TryWrite, Status;

PROCEDURE StreamSinkProc(ctx: ADDRESS; VAR rec: Record);
VAR
  buf: LineBuf;
  len, wrote: INTEGER;
  st: Status;
BEGIN
  IF ctx = NIL THEN RETURN END;

  Format(rec, buf, len);

  (* Append newline *)
  IF len < MaxLine - 1 THEN
    buf[len] := 12C;
    INC(len);
    buf[len] := 0C
  END;

  (* Try to write; silently drop on failure or WouldBlock *)
  st := TryWrite(ctx, ADR(buf), len, wrote)
END StreamSinkProc;

PROCEDURE Create(streamHandle: ADDRESS; VAR out: Sink);
BEGIN
  out.proc := StreamSinkProc;
  out.ctx := streamHandle;
  out.minLevel := TRACE
END Create;

END LogSinkStream.
