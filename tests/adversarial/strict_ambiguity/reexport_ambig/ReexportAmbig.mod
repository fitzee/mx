MODULE ReexportAmbig;
FROM RA_A IMPORT Compute;
FROM RA_C IMPORT Compute;
VAR result: INTEGER;
BEGIN
  result := Compute(5)
END ReexportAmbig.
