MODULE ParamEntryClash;
(* Regression: a parameter named "entry" clashes with the LLVM IR
   entry block label, causing "unable to create block named 'entry'".
   Also tests other LLVM-reserved names as parameters. *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;

PROCEDURE Lookup(entry: INTEGER; VAR ok: BOOLEAN);
BEGIN
  ok := (entry > 0);
  WriteString("entry=");
  WriteInt(entry, 0);
  WriteLn
END Lookup;

PROCEDURE SetValue(VAR entry: INTEGER; val: INTEGER);
BEGIN
  entry := val
END SetValue;

VAR
  n: INTEGER;
  found: BOOLEAN;

BEGIN
  Lookup(42, found);
  IF found THEN
    WriteString("found=TRUE")
  ELSE
    WriteString("found=FALSE")
  END;
  WriteLn;

  n := 0;
  SetValue(n, 99);
  WriteString("n=");
  WriteInt(n, 0);
  WriteLn
END ParamEntryClash.
