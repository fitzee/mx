MODULE VarAddrLeakMain;
(* Regression test: array_vars scope leak across embedded procedures.

   ArrayKeyLib.LookupName has a local key: ARRAY [0..63] OF CHAR.
   AddrKeyLib.GetHandle has VAR key: ADDRESS as an out parameter.

   Bug: the name "key" from ArrayKeyLib leaked into array_vars,
   causing GetHandle's  key := store.ptr  to emit memcpy
   instead of a direct pointer assignment.

   This corrupted the returned pointer, causing a crash when
   the caller dereferenced it. *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADDRESS;
FROM ArrayKeyLib IMPORT LookupName;
FROM AddrKeyLib IMPORT GetHandle;

VAR
  found: BOOLEAN;
  handle: ADDRESS;
  ok: BOOLEAN;

BEGIN
  (* Exercise ArrayKeyLib first (populates array_vars with "key") *)
  LookupName("hello", found);
  IF found THEN
    WriteString("found=yes")
  ELSE
    WriteString("found=no")
  END;
  WriteLn;

  (* Now call AddrKeyLib — if "key" leaked, this crashes *)
  ok := GetHandle(42, handle);
  IF ok AND (handle # NIL) THEN
    WriteString("handle=ok")
  ELSE
    WriteString("handle=FAIL")
  END;
  WriteLn;

  (* Negative case *)
  ok := GetHandle(99, handle);
  IF ok THEN
    WriteString("miss=FAIL")
  ELSE
    WriteString("miss=no")
  END;
  WriteLn;

  WriteString("done"); WriteLn
END VarAddrLeakMain.
