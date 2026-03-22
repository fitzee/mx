IMPLEMENTATION MODULE Builder;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Concat, Length;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Sys IMPORT m2sys_exec;

VAR
  cmd: ARRAY [0..4095] OF CHAR;
  tmp: ARRAY [0..4095] OF CHAR;

PROCEDURE Append(s: ARRAY OF CHAR);
BEGIN
  Concat(cmd, s, tmp);
  Assign(tmp, cmd)
END Append;

PROCEDURE Build(release: INTEGER; target: ARRAY OF CHAR);
VAR rc: INTEGER;
BEGIN
  Assign("mx build", cmd);
  IF release = 1 THEN
    Append(" --release")
  END;
  IF Length(target) > 0 THEN
    Append(" --target ");
    Append(target)
  END;

  rc := m2sys_exec(ADR(cmd));
  IF rc # 0 THEN
    RAISE BuildError
  END
END Build;

PROCEDURE BuildAndRun(release: INTEGER; target: ARRAY OF CHAR);
VAR rc: INTEGER;
BEGIN
  Assign("mx run", cmd);
  IF release = 1 THEN
    Append(" --release")
  END;
  IF Length(target) > 0 THEN
    Append(" --target ");
    Append(target)
  END;

  rc := m2sys_exec(ADR(cmd));
  IF rc # 0 THEN
    RAISE BuildError
  END
END BuildAndRun;

END Builder.
