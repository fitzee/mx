MODULE TypecastDerefAssign;
(* Regression: TypeName(expr)^ := val must parse and execute correctly.
   The parser was treating TypeName(expr) as a procedure call statement,
   then choking on ^ after the closing paren.

   Tests:
     1. CharPtr(addr)^ := val    -- byte write via pointer cast
     2. CharPtr(expr)^           -- cast of computed address
     3. IntPtr(addr)^            -- 4-byte write via pointer cast
     4. RecPtr(addr)^.field      -- cast + deref + field access
     5. Read back all values to verify correctness *)

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  CharPtr = POINTER TO CHAR;
  IntPtr  = POINTER TO INTEGER;
  Rec = RECORD
    x: INTEGER;
    y: INTEGER;
  END;
  RecPtr = POINTER TO Rec;

VAR
  buf: ARRAY [0..31] OF CHAR;
  r: Rec;
  p: ADDRESS;
  ip: IntPtr;
  rp: RecPtr;
  ch: CHAR;
  ok: BOOLEAN;

BEGIN
  (* Test 1: CharPtr(addr)^ write *)
  p := ADR(buf);
  CharPtr(p)^ := 'H';
  CharPtr(LONGCARD(p) + 1)^ := 'i';
  CharPtr(LONGCARD(p) + 2)^ := 0C;
  WriteString("t1="); WriteString(buf); WriteLn;

  (* Test 2: read back via CharPtr cast *)
  ch := CharPtr(p)^;
  IF ch = 'H' THEN
    WriteString("t2=ok")
  ELSE
    WriteString("t2=FAIL")
  END;
  WriteLn;

  (* Test 3: IntPtr cast write *)
  ip := ADR(r);
  IntPtr(ADR(r.x))^ := 42;
  IF r.x = 42 THEN
    WriteString("t3=ok")
  ELSE
    WriteString("t3=FAIL")
  END;
  WriteLn;

  (* Test 4: RecPtr(addr)^.field write *)
  rp := ADR(r);
  RecPtr(rp)^.x := 100;
  RecPtr(rp)^.y := 200;
  IF (r.x = 100) AND (r.y = 200) THEN
    WriteString("t4=ok")
  ELSE
    WriteString("t4=FAIL")
  END;
  WriteLn;

  (* Test 5: combined — write via cast, read normally *)
  IntPtr(ADR(r.y))^ := 999;
  WriteString("t5="); WriteInt(r.y, 0); WriteLn
END TypecastDerefAssign.
