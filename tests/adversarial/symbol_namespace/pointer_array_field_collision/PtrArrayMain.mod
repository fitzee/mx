MODULE PtrArrayMain;
(* Regression test: two embedded modules both have a field named 'data',
   one is an array (memcpy on assign) and one is a pointer (simple assign).
   Bug: codegen's is_array_field matched by bare name, causing pointer
   fields to get memcpy instead of pointer assignment -> SIGSEGV. *)
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM SYSTEM IMPORT ADR;
IMPORT ArrayRec;
IMPORT PtrRec;

VAR
  ar: ArrayRec.ArrBuf;
  pr: PtrRec.PtrBuf;
  buf: ARRAY [0..255] OF CHAR;
  b: INTEGER;

BEGIN
  (* Test array record: data is array, assignment should use memcpy *)
  ArrayRec.InitArr(ar);
  WriteString("sum="); WriteInt(ArrayRec.SumArr(ar), 1); WriteLn;

  (* Test pointer record: data is pointer, assignment must NOT memcpy *)
  buf[0] := CHR(65);
  buf[1] := CHR(66);
  buf[2] := CHR(67);
  PtrRec.InitPtr(pr, ADR(buf), 3);
  b := PtrRec.ReadByte(pr, 0);
  WriteString("byte0="); WriteInt(b, 1); WriteLn;
  b := PtrRec.ReadByte(pr, 2);
  WriteString("byte2="); WriteInt(b, 1); WriteLn;

  WriteString("ok"); WriteLn
END PtrArrayMain.
