MODULE FuncCallIndex;
(* Regression: function call used as array index on LHS of assignment
   was emitted as 0 in C backend, silently writing to index 0 instead
   of the computed index. *)

FROM InOut IMPORT WriteString, WriteLn;

VAR
  buf: ARRAY [0..15] OF CHAR;
  pos: INTEGER;

PROCEDURE GetPos(): INTEGER;
BEGIN
  RETURN pos;
END GetPos;

PROCEDURE SetAt(i: INTEGER; ch: CHAR);
BEGIN
  buf[GetPos()] := ch;
END SetAt;

BEGIN
  buf[0] := '-';
  buf[1] := '-';
  buf[2] := '-';
  buf[3] := '-';
  buf[4] := 0C;

  (* Write 'X' at position 2 via function-call index *)
  pos := 2;
  SetAt(pos, 'X');

  (* buf should be "--X-" not "X---" *)
  IF buf[0] = '-' THEN
    WriteString("ok0")
  ELSE
    WriteString("FAIL0")
  END;
  WriteLn;

  IF buf[2] = 'X' THEN
    WriteString("ok2")
  ELSE
    WriteString("FAIL2")
  END;
  WriteLn;

  (* Also test direct function-call index in module body *)
  pos := 3;
  buf[GetPos()] := 'Y';
  IF buf[3] = 'Y' THEN
    WriteString("ok3")
  ELSE
    WriteString("FAIL3")
  END;
  WriteLn
END FuncCallIndex.
