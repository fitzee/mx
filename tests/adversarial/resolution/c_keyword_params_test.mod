MODULE c_keyword_params_test;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

PROCEDURE DoStuff(short: INTEGER; long: INTEGER; register: INTEGER): INTEGER;
BEGIN
  RETURN short + long + register
END DoStuff;

PROCEDURE WithArrayParam(default: ARRAY OF CHAR);
BEGIN
  WriteString(default); WriteLn
END WithArrayParam;

VAR result: INTEGER;

BEGIN
  result := DoStuff(10, 20, 30);
  IF result = 60 THEN
    WriteString("keyword params OK"); WriteLn
  END;
  WithArrayParam("hello from default");
  WriteString("all keyword params OK"); WriteLn
END c_keyword_params_test.
