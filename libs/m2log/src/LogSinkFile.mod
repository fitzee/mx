IMPLEMENTATION MODULE LogSinkFile;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Log IMPORT Sink, Record, Level, Format, LineBuf, MaxLine;
FROM Sys IMPORT m2sys_fopen, m2sys_fclose, m2sys_fwrite_str;

TYPE
  HandlePtr = POINTER TO HandleRec;
  HandleRec = RECORD
    fh: INTEGER;
  END;

PROCEDURE FileSinkProc(ctx: ADDRESS; VAR rec: Record);
VAR
  buf: LineBuf;
  len: INTEGER;
  hp: HandlePtr;
  nl: ARRAY [0..1] OF CHAR;
BEGIN
  hp := ctx;
  IF (hp = NIL) OR (hp^.fh < 0) THEN RETURN END;

  Format(rec, buf, len);
  m2sys_fwrite_str(hp^.fh, ADR(buf));
  nl[0] := 12C;
  nl[1] := 0C;
  m2sys_fwrite_str(hp^.fh, ADR(nl))
END FileSinkProc;

PROCEDURE Create(path: ARRAY OF CHAR; VAR out: Sink): BOOLEAN;
VAR
  mode: ARRAY [0..1] OF CHAR;
  hp: HandlePtr;
  fh: INTEGER;
BEGIN
  mode[0] := 'a';
  mode[1] := 0C;
  fh := m2sys_fopen(ADR(path), ADR(mode));
  IF fh < 0 THEN RETURN FALSE END;

  ALLOCATE(hp, TSIZE(HandleRec));
  hp^.fh := fh;

  out.proc := FileSinkProc;
  out.ctx := hp;
  out.minLevel := TRACE;
  RETURN TRUE
END Create;

PROCEDURE Close(VAR s: Sink);
VAR hp: HandlePtr;
BEGIN
  hp := s.ctx;
  IF hp # NIL THEN
    IF hp^.fh >= 0 THEN
      m2sys_fclose(hp^.fh);
      hp^.fh := -1
    END;
    DEALLOCATE(hp, TSIZE(HandleRec));
    s.ctx := NIL
  END;
  s.proc := NIL
END Close;

END LogSinkFile.
