MODULE proc_var_call_test;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
  Data = RECORD val: INTEGER END;
  HandlerProc = PROCEDURE(VAR Data, INTEGER);

  Entry = RECORD
    handler: HandlerProc
  END;

PROCEDURE MyHandler(VAR d: Data; n: INTEGER);
BEGIN
  d.val := d.val + n
END MyHandler;

VAR
  d: Data;
  h: HandlerProc;
  e: Entry;

BEGIN
  d.val := 10;

  (* Call through simple proc-typed variable *)
  h := MyHandler;
  h(d, 5);
  IF d.val = 15 THEN
    WriteString("proc var call OK"); WriteLn
  END;

  (* Call through record field proc-typed variable *)
  e.handler := MyHandler;
  e.handler(d, 7);
  IF d.val = 22 THEN
    WriteString("record field proc call OK"); WriteLn
  END;

  WriteString("all proc var calls OK"); WriteLn
END proc_var_call_test.
