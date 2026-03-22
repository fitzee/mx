MODULE Main;
(* m2dap — Modula-2 Debug Adapter Protocol server.
   Reads DAP messages from stdin, dispatches to handlers,
   writes DAP responses to stdout. *)

FROM DAPTransport IMPORT ReadMessage, WriteMessage;
FROM DAPServer IMPORT HandleMessage;
FROM Sys IMPORT m2sys_exit;

CONST
  MaxMsg = 65536;

VAR
  buf: ARRAY [0..MaxMsg-1] OF CHAR;
  len: CARDINAL;
  running: BOOLEAN;

BEGIN
  running := TRUE;
  WHILE running DO
    IF ReadMessage(buf, len) THEN
      running := HandleMessage(buf, len)
    ELSE
      running := FALSE
    END
  END;
  m2sys_exit(0)
END Main.
