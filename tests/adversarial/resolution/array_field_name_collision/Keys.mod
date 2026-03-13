IMPLEMENTATION MODULE Keys;

FROM Strings IMPORT Assign;

PROCEDURE InitEntry(VAR e: KeyEntry);
BEGIN
  e.tag[0] := 0C;
  e.active := FALSE
END InitEntry;

END Keys.
