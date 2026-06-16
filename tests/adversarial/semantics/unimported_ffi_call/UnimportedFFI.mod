MODULE UnimportedFFI;
(* Regression: calling a procedure from a DEFINITION MODULE FOR "C"
   that exists in the def but was NOT imported via FROM M IMPORT
   must produce a compile error, not silently compile and return 0. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_file_exists;  (* m2sys_list_dir NOT imported *)

VAR
  buf: ARRAY [0..511] OF CHAR;
  rc: INTEGER;
BEGIN
  rc := m2sys_list_dir(ADR("/tmp"), ADR(buf), 512);
  WriteString("should not reach here"); WriteLn
END UnimportedFFI.
