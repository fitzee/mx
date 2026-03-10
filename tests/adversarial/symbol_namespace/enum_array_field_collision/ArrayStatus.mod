IMPLEMENTATION MODULE ArrayStatus;

FROM Strings IMPORT Assign;

PROCEDURE InitLog(VAR e: LogEntry; s: ARRAY OF CHAR; c: CARDINAL);
BEGIN
  Assign(s, e.status);
  e.code := c
END InitLog;

END ArrayStatus.
