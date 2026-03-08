IMPLEMENTATION MODULE ArrayKeyLib;

FROM Strings IMPORT Assign;

PROCEDURE LookupName(VAR name: ARRAY OF CHAR; VAR found: BOOLEAN);
VAR
  key: ARRAY [0..63] OF CHAR;
BEGIN
  Assign(name, key);
  found := (key[0] # 0C)
END LookupName;

END ArrayKeyLib.
