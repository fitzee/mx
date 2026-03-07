IMPLEMENTATION MODULE ImportAsMulti;

FROM InOut IMPORT WriteString, WriteLn;

PROCEDURE Greet(msg: ARRAY OF CHAR);
BEGIN
  WriteString(msg);
  WriteLn;
END Greet;

PROCEDURE Add(a, b: INTEGER): INTEGER;
BEGIN
  RETURN a + b;
END Add;

END ImportAsMulti.
