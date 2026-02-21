MODULE ShadowLocal;
(* Tests that a local variable named "value" can coexist with
   Shadow_A.value (an exported VAR) via qualified import.
   Local names do not collide with module-qualified names. *)
FROM InOut IMPORT WriteInt, WriteLn;
IMPORT Shadow_A;
VAR value: INTEGER;
BEGIN
  value := 42;
  WriteInt(value, 0); WriteLn;
  WriteInt(Shadow_A.value, 0); WriteLn
END ShadowLocal.
