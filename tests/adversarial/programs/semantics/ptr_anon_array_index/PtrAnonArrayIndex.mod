MODULE PtrAnonArrayIndex;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADR;

TYPE
  BufPtr = POINTER TO ARRAY [0..7] OF INTEGER;

VAR
  buf: ARRAY [0..7] OF INTEGER;
  bp: BufPtr;
  i: INTEGER;

BEGIN
  (* Fill array *)
  FOR i := 0 TO 7 DO
    buf[i] := (i + 1) * 10
  END;

  (* Take pointer to array *)
  bp := ADR(buf);

  (* Index through pointer: bp^[i] must generate bp[i] *)
  WriteString("v0="); WriteInt(bp^[0], 0); WriteLn;
  WriteString("v3="); WriteInt(bp^[3], 0); WriteLn;
  WriteString("v7="); WriteInt(bp^[7], 0); WriteLn;

  (* Modify through pointer *)
  bp^[4] := 999;
  WriteString("m4="); WriteInt(buf[4], 0); WriteLn;

  (* Loop through pointer *)
  i := 0;
  FOR i := 0 TO 7 DO
    IF bp^[i] > 50 THEN
      WriteString("gt50="); WriteInt(bp^[i], 0); WriteLn
    END
  END
END PtrAnonArrayIndex.
