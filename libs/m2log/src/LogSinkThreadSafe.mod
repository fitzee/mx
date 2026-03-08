IMPLEMENTATION MODULE LogSinkThreadSafe;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Log IMPORT Sink, Record, Level, Format, LineBuf, MaxLine;
FROM Sys IMPORT m2sys_fopen, m2sys_fclose, m2sys_fwrite_str;
FROM Threads IMPORT Mutex, MutexInit, MutexDestroy, MutexLock, MutexUnlock;

TYPE
  CtxPtr = POINTER TO CtxRec;
  CtxRec = RECORD
    mu:     Mutex;
    fh:     INTEGER;
    ownsFh: BOOLEAN;
  END;

PROCEDURE SinkProc(ctx: ADDRESS; VAR rec: Record);
VAR
  cp: CtxPtr;
  buf: LineBuf;
  len: INTEGER;
  nl: ARRAY [0..1] OF CHAR;
BEGIN
  cp := ctx;
  IF cp = NIL THEN RETURN END;
  MutexLock(cp^.mu);
  Format(rec, buf, len);
  m2sys_fwrite_str(cp^.fh, ADR(buf));
  nl[0] := 12C;
  nl[1] := 0C;
  m2sys_fwrite_str(cp^.fh, ADR(nl));
  MutexUnlock(cp^.mu)
END SinkProc;

PROCEDURE CreateFile(path: ARRAY OF CHAR; VAR out: Sink): BOOLEAN;
VAR
  mode: ARRAY [0..1] OF CHAR;
  cp: CtxPtr;
  fh: INTEGER;
BEGIN
  mode[0] := 'a';
  mode[1] := 0C;
  fh := m2sys_fopen(ADR(path), ADR(mode));
  IF fh < 0 THEN RETURN FALSE END;

  ALLOCATE(cp, TSIZE(CtxRec));
  cp^.fh := fh;
  cp^.ownsFh := TRUE;
  MutexInit(cp^.mu);

  out.proc := SinkProc;
  out.ctx := cp;
  out.minLevel := TRACE;
  RETURN TRUE
END CreateFile;

PROCEDURE CreateStderr(VAR out: Sink);
VAR cp: CtxPtr;
BEGIN
  ALLOCATE(cp, TSIZE(CtxRec));
  cp^.fh := 2;
  cp^.ownsFh := FALSE;
  MutexInit(cp^.mu);

  out.proc := SinkProc;
  out.ctx := cp;
  out.minLevel := TRACE
END CreateStderr;

PROCEDURE Close(VAR s: Sink);
VAR cp: CtxPtr;
BEGIN
  cp := s.ctx;
  IF cp # NIL THEN
    MutexDestroy(cp^.mu);
    IF cp^.ownsFh AND (cp^.fh >= 0) THEN
      m2sys_fclose(cp^.fh);
      cp^.fh := -1
    END;
    DEALLOCATE(cp, TSIZE(CtxRec));
    s.ctx := NIL
  END;
  s.proc := NIL
END Close;

END LogSinkThreadSafe.
