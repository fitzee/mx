IMPLEMENTATION MODULE LogSinkMemory;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Strings IMPORT Assign, Pos;
FROM Log IMPORT Sink, Record, Level, Format, LineBuf, MaxLine;

PROCEDURE MemorySinkProc(ctx: ADDRESS; VAR rec: Record);
VAR
  buf: LineBuf;
  len, i: INTEGER;
  mp: POINTER TO MemorySink;
BEGIN
  mp := ctx;
  IF mp = NIL THEN RETURN END;

  Format(rec, buf, len);

  i := 0;
  WHILE (i < LineLen - 1) AND (i < len) AND (buf[i] # 0C) DO
    mp^.lines[mp^.nextSlot][i] := buf[i];
    INC(i)
  END;
  mp^.lines[mp^.nextSlot][i] := 0C;

  mp^.nextSlot := (mp^.nextSlot + 1) MOD MaxLines;
  INC(mp^.totalSeen);
  IF mp^.count < MaxLines THEN
    INC(mp^.count)
  END
END MemorySinkProc;

PROCEDURE Create(VAR mem: MemorySink; VAR out: Sink);
BEGIN
  Clear(mem);
  out.proc := MemorySinkProc;
  out.ctx := ADR(mem);
  out.minLevel := TRACE
END Create;

PROCEDURE GetCount(VAR mem: MemorySink): INTEGER;
BEGIN
  RETURN mem.count
END GetCount;

PROCEDURE GetTotal(VAR mem: MemorySink): INTEGER;
BEGIN
  RETURN mem.totalSeen
END GetTotal;

PROCEDURE GetLine(VAR mem: MemorySink; index: INTEGER;
                  VAR buf: ARRAY OF CHAR): BOOLEAN;
VAR slot: INTEGER;
BEGIN
  IF (index < 0) OR (index >= mem.count) THEN
    buf[0] := 0C;
    RETURN FALSE
  END;

  IF mem.count < MaxLines THEN
    slot := index
  ELSE
    slot := (mem.nextSlot + index) MOD MaxLines
  END;

  Assign(mem.lines[slot], buf);
  RETURN TRUE
END GetLine;

PROCEDURE Clear(VAR mem: MemorySink);
BEGIN
  mem.count := 0;
  mem.nextSlot := 0;
  mem.totalSeen := 0
END Clear;

PROCEDURE Contains(VAR mem: MemorySink;
                   sub: ARRAY OF CHAR): BOOLEAN;
VAR
  i, slot: INTEGER;
  p: CARDINAL;
BEGIN
  i := 0;
  WHILE i < mem.count DO
    IF mem.count < MaxLines THEN
      slot := i
    ELSE
      slot := (mem.nextSlot + i) MOD MaxLines
    END;
    p := Pos(sub, mem.lines[slot]);
    IF p < CARDINAL(LineLen) THEN
      RETURN TRUE
    END;
    INC(i)
  END;
  RETURN FALSE
END Contains;

END LogSinkMemory.
