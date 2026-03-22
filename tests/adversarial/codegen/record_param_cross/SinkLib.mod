IMPLEMENTATION MODULE SinkLib;
FROM SYSTEM IMPORT ADDRESS;

PROCEDURE Init(VAR l: Logger);
BEGIN
  l.count := 0
END Init;

PROCEDURE MakeSink(cb: Callback; level: INTEGER): Sink;
VAR s: Sink;
BEGIN
  s.proc := cb;
  s.ctx := NIL;
  s.level := level;
  RETURN s
END MakeSink;

PROCEDURE AddSink(VAR l: Logger; s: Sink): BOOLEAN;
BEGIN
  IF l.count >= 4 THEN RETURN FALSE END;
  l.sinks[l.count] := s;
  INC(l.count);
  RETURN TRUE
END AddSink;

PROCEDURE Dispatch(VAR l: Logger; msg: INTEGER);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE i < l.count DO
    IF msg >= l.sinks[i].level THEN
      l.sinks[i].proc(l.sinks[i].ctx, msg)
    END;
    INC(i)
  END
END Dispatch;

END SinkLib.
