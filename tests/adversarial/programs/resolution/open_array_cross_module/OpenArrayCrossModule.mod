MODULE OpenArrayCrossModule;
(* Regression test: when two imported modules both export a procedure named
   "Init", the FROM-import must resolve to the correct module's param info.
   Previously, symtab.lookup_any("Init") could find the wrong module's Init
   (e.g., Config.Init with no open array) instead of Encoder.Init, causing
   the open-array high bound to be omitted from the generated C call. *)

FROM InOut IMPORT WriteString, WriteLn;
FROM Encoder IMPORT Init;
FROM Config IMPORT Settings;

VAR
  buf: ARRAY [0..9] OF CHAR;
  cfg: Settings;

PROCEDURE RunWithOpenArray(VAR data: ARRAY OF CHAR);
BEGIN
  (* open array forwarded to open array — high bound must propagate *)
  Init(data, 5);
END RunWithOpenArray;

BEGIN
  (* fixed array passed to open array — high bound must be injected *)
  Init(buf, 10);
  IF buf[0] = 'A' THEN
    WriteString("fixed array high OK")
  END;
  WriteLn;
  IF buf[9] = 'J' THEN
    WriteString("fixed array fill OK")
  END;
  WriteLn;

  RunWithOpenArray(buf);
  IF buf[0] = 'A' THEN
    WriteString("open array forward OK")
  END;
  WriteLn;

  WriteString("all open array cross module OK");
  WriteLn
END OpenArrayCrossModule.
