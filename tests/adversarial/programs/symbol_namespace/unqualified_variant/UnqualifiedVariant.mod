MODULE UnqualifiedVariant;
FROM InOut IMPORT WriteString, WriteLn;
TYPE
  Color = (Red, Green, Blue);
  Status = (OK, Pending, Failed);
VAR
  c: Color;
  s: Status;
BEGIN
  c := Red;
  IF c = Red THEN WriteString("Red"); WriteLn END;
  c := Blue;
  IF c = Blue THEN WriteString("Blue"); WriteLn END;
  s := OK;
  IF s = OK THEN WriteString("OK"); WriteLn END;
  s := Pending;
  IF s = Pending THEN WriteString("Pending"); WriteLn END;
  s := Failed;
  IF s = Failed THEN WriteString("Failed"); WriteLn END
END UnqualifiedVariant.
