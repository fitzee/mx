IMPLEMENTATION MODULE Process;
FROM SYSTEM IMPORT ADR, ADDRESS;
FROM ProcessBridge IMPORT m2dap_spawn, m2dap_read, m2dap_write,
                          m2dap_close, m2dap_kill, m2dap_waitpid;

CONST
  SentinelLen = 7;  (* length of "(m2dap)" *)

VAR
  pid: INTEGER;
  stdinFd: INTEGER;
  stdoutFd: INTEGER;
  stderrFd: INTEGER;
  running: BOOLEAN;

  (* Read buffer for lldb stdout *)
  rBuf: ARRAY [0..4095] OF CHAR;
  rPos: CARDINAL;
  rLen: CARDINAL;

PROCEDURE FillBuf(): BOOLEAN;
VAR n: INTEGER;
BEGIN
  n := m2dap_read(stdoutFd, ADR(rBuf), 4096);
  IF n <= 0 THEN RETURN FALSE END;
  rPos := 0;
  rLen := VAL(CARDINAL, n);
  RETURN TRUE
END FillBuf;

PROCEDURE GetByte(VAR ch: CHAR): BOOLEAN;
BEGIN
  IF rPos >= rLen THEN
    IF NOT FillBuf() THEN RETURN FALSE END
  END;
  ch := rBuf[rPos];
  INC(rPos);
  RETURN TRUE
END GetByte;

PROCEDURE WriteFd(fd: INTEGER; buf: ADDRESS; len: CARDINAL);
VAR n: INTEGER;
BEGIN
  n := m2dap_write(fd, buf, VAL(INTEGER, len))
END WriteFd;

PROCEDURE Spawn(): BOOLEAN;
VAR
  prog: ARRAY [0..31] OF CHAR;
  args: ARRAY [0..31] OF CHAR;
  initCmd: ARRAY [0..63] OF CHAR;
  ok: BOOLEAN;
  dummy: ARRAY [0..4095] OF CHAR;
  dLen: CARDINAL;
BEGIN
  prog := "lldb";
  args := "--no-use-colors";
  pid := m2dap_spawn(ADR(prog), ADR(args),
                     stdinFd, stdoutFd, stderrFd);
  IF pid < 0 THEN RETURN FALSE END;
  running := TRUE;
  rPos := 0;
  rLen := 0;

  (* Set custom prompt so we can detect response boundaries *)
  initCmd := 'settings set prompt "(m2dap) "';
  SendLine(initCmd);

  (* Read past initial lldb banner + first prompt *)
  ok := ReadUntilPrompt(dummy, dLen);

  RETURN ok
END Spawn;

PROCEDURE SendLine(VAR cmd: ARRAY OF CHAR);
VAR
  i: CARDINAL;
  nl: CHAR;
BEGIN
  (* Find string length *)
  i := 0;
  WHILE (i <= HIGH(cmd)) AND (cmd[i] # CHR(0)) DO
    INC(i)
  END;
  WriteFd(stdinFd, ADR(cmd), i);
  nl := CHR(10);
  WriteFd(stdinFd, ADR(nl), 1)
END SendLine;

PROCEDURE ReadUntilPrompt(VAR buf: ARRAY OF CHAR;
                          VAR len: CARDINAL): BOOLEAN;
(* Read bytes until we see "(m2dap) " (7 chars + space).
   The sentinel is NOT included in the output. *)
VAR
  ch: CHAR;
  pos: CARDINAL;
  sentinel: ARRAY [0..7] OF CHAR;
  matchLen: CARDINAL;
  i: CARDINAL;
BEGIN
  sentinel := "(m2dap) ";
  matchLen := 8;

  pos := 0;
  LOOP
    IF NOT GetByte(ch) THEN
      len := pos;
      IF pos <= HIGH(buf) THEN buf[pos] := CHR(0) END;
      RETURN FALSE
    END;

    IF pos <= HIGH(buf) THEN
      buf[pos] := ch
    END;
    INC(pos);

    (* Check if last matchLen bytes match sentinel *)
    IF pos >= matchLen THEN
      i := 0;
      WHILE (i < matchLen) AND
            (buf[pos - matchLen + i] = sentinel[i]) DO
        INC(i)
      END;
      IF i = matchLen THEN
        (* Found sentinel — remove it from output *)
        pos := pos - matchLen;
        (* Also strip trailing newline before prompt if present *)
        IF (pos > 0) AND (buf[pos-1] = CHR(10)) THEN DEC(pos) END;
        IF (pos > 0) AND (buf[pos-1] = CHR(13)) THEN DEC(pos) END;
        len := pos;
        IF pos <= HIGH(buf) THEN buf[pos] := CHR(0) END;
        RETURN TRUE
      END
    END
  END
END ReadUntilPrompt;

PROCEDURE Kill;
VAR n: INTEGER;
BEGIN
  IF running THEN
    n := m2dap_kill(pid, 15);  (* SIGTERM *)
    n := m2dap_waitpid(pid, 1);
    m2dap_close(stdinFd);
    m2dap_close(stdoutFd);
    m2dap_close(stderrFd);
    running := FALSE
  END
END Kill;

PROCEDURE IsRunning(): BOOLEAN;
BEGIN
  RETURN running
END IsRunning;

BEGIN
  pid := -1;
  stdinFd := -1;
  stdoutFd := -1;
  stderrFd := -1;
  running := FALSE;
  rPos := 0;
  rLen := 0
END Process.
