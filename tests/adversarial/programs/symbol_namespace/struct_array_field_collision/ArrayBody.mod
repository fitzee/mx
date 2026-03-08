IMPLEMENTATION MODULE ArrayBody;

FROM Strings IMPORT Assign;

PROCEDURE InitPacket(VAR p: Packet; data: ARRAY OF CHAR; n: CARDINAL);
BEGIN
  Assign(data, p.body);
  p.len := n
END InitPacket;

END ArrayBody.
