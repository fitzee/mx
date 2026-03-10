MODULE ReexportMain;
FROM InOut IMPORT WriteInt, WriteLn;
FROM ChainC IMPORT Process;
VAR r: INTEGER;
BEGIN
  r := Process(5);
  (* 5*2=10, 10+1=11, 11+10=21 *)
  WriteInt(r, 0); WriteLn
END ReexportMain.
