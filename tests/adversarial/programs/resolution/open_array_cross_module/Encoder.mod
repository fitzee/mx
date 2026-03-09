IMPLEMENTATION MODULE Encoder;

PROCEDURE Init(VAR buf: ARRAY OF CHAR; len: CARDINAL);
VAR i: CARDINAL;
BEGIN
  IF len > HIGH(buf) + 1 THEN
    len := HIGH(buf) + 1
  END;
  i := 0;
  WHILE i < len DO
    buf[i] := CHR(ORD('A') + (i MOD 26));
    INC(i)
  END
END Init;

END Encoder.
