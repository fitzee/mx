IMPLEMENTATION MODULE ArrayStatus;

FROM Strings IMPORT Assign;

PROCEDURE InitMsg(VAR m: Msg; s: ARRAY OF CHAR; c: INTEGER);
BEGIN
  Assign(s, m.status);
  m.code := c
END InitMsg;

END ArrayStatus.
