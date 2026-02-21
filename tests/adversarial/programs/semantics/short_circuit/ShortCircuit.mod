MODULE ShortCircuit;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
VAR counter: INTEGER;

PROCEDURE Bump(): BOOLEAN;
BEGIN
  counter := counter + 1;
  RETURN TRUE
END Bump;

PROCEDURE Fail(): BOOLEAN;
BEGIN
  counter := counter + 1;
  RETURN FALSE
END Fail;

BEGIN
  (* Test 1: FALSE AND Bump() — Bump should NOT be called *)
  counter := 0;
  IF FALSE AND Bump() THEN END;
  WriteString("T1:"); WriteInt(counter, 0); WriteLn;

  (* Test 2: TRUE OR Bump() — Bump should NOT be called *)
  counter := 0;
  IF TRUE OR Bump() THEN END;
  WriteString("T2:"); WriteInt(counter, 0); WriteLn;

  (* Test 3: TRUE AND Bump() — Bump SHOULD be called *)
  counter := 0;
  IF TRUE AND Bump() THEN END;
  WriteString("T3:"); WriteInt(counter, 0); WriteLn;

  (* Test 4: FALSE OR Bump() — Bump SHOULD be called *)
  counter := 0;
  IF FALSE OR Bump() THEN END;
  WriteString("T4:"); WriteInt(counter, 0); WriteLn;

  (* Test 5: nested — FALSE AND (Bump() OR Bump()) *)
  counter := 0;
  IF FALSE AND (Bump() OR Bump()) THEN END;
  WriteString("T5:"); WriteInt(counter, 0); WriteLn;

  (* Test 6: TRUE OR (Fail() AND Fail()) *)
  counter := 0;
  IF TRUE OR (Fail() AND Fail()) THEN END;
  WriteString("T6:"); WriteInt(counter, 0); WriteLn
END ShortCircuit.
