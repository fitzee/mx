MODULE CStdlibClash;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  log: INTEGER;
  time: INTEGER;
  stat: INTEGER;
  errno: INTEGER;
  read: INTEGER;
  write: INTEGER;

PROCEDURE signal(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 2
END signal;

BEGIN
  log := 10;
  time := 20;
  stat := 30;
  errno := 40;
  read := 50;
  write := 60;

  WriteString("log="); WriteInt(log, 0); WriteLn;
  WriteString("time="); WriteInt(time, 0); WriteLn;
  WriteString("stat="); WriteInt(stat, 0); WriteLn;
  WriteString("errno="); WriteInt(errno, 0); WriteLn;
  WriteString("read="); WriteInt(read, 0); WriteLn;
  WriteString("write="); WriteInt(write, 0); WriteLn;
  WriteString("signal="); WriteInt(signal(7), 0); WriteLn
END CStdlibClash.
