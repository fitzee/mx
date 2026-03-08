MODULE ScalarArrayMain;
(* Regression test: two imported records both have a field named 'status',
   one is ARRAY OF CHAR (memcpy on assign) and one is CARDINAL (direct assign).
   Bug: codegen matched array_fields by bare field name, causing scalar
   fields to get memcpy instead of direct assignment -> SIGSEGV at runtime
   (e.g., memcpy(resp.status, 200, sizeof(resp.status)) treats 200 as a
   pointer).

   This test covers:
   - simple var.field := scalar
   - array[idx].field := scalar  (the deeper nesting case)
   - array[idx].field := variable
*)
FROM InOut IMPORT WriteString, WriteInt, WriteCard, WriteLn;
FROM ArrayStatus IMPORT Msg, InitMsg;
FROM ScalarStatus IMPORT Response, InitResp, GetStatus;

VAR
  m: Msg;
  r: Response;
  batch: ARRAY [0..3] OF Response;
  code: CARDINAL;
  i: INTEGER;

BEGIN
  (* Array-typed status field: assign should use memcpy/strcpy *)
  InitMsg(m, "OK", 200);
  WriteString("msg="); WriteString(m.status); WriteLn;
  WriteString("code="); WriteInt(m.code, 1); WriteLn;

  (* Scalar-typed status field: assign MUST be direct, not memcpy *)
  InitResp(r, 404, 0);
  WriteString("status="); WriteCard(GetStatus(r), 1); WriteLn;

  (* Direct scalar field assignment in main module *)
  r.status := 200;
  WriteString("status2="); WriteCard(r.status, 1); WriteLn;

  (* Assign from variable *)
  code := 301;
  r.status := code;
  WriteString("status3="); WriteCard(r.status, 1); WriteLn;

  (* Array-indexed record field := literal (the deep nesting case) *)
  batch[0].status := 500;
  WriteString("batch0="); WriteCard(batch[0].status, 1); WriteLn;

  (* Array-indexed record field := variable *)
  code := 401;
  batch[1].status := code;
  WriteString("batch1="); WriteCard(batch[1].status, 1); WriteLn;

  (* Loop over array assigning to scalar field *)
  FOR i := 0 TO 3 DO
    batch[i].status := CARDINAL(i) + 100
  END;
  WriteString("batch2="); WriteCard(batch[2].status, 1); WriteLn;
  WriteString("batch3="); WriteCard(batch[3].status, 1); WriteLn;

  WriteString("ok"); WriteLn
END ScalarArrayMain.
