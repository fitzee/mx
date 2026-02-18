IMPLEMENTATION MODULE Log;

FROM InOut IMPORT WriteString, WriteLn;

PROCEDURE Info(msg: ARRAY OF CHAR);
BEGIN
  WriteString("[INFO] ");
  WriteString(msg);
  WriteLn
END Info;

PROCEDURE Error(msg: ARRAY OF CHAR);
BEGIN
  WriteString("[ERROR] ");
  WriteString(msg);
  WriteLn
END Error;

END Log.
