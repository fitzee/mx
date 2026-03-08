MODULE OpenArrayScalarMain;
(* Regression test: assigning a CARDINAL field to a VAR open array element's
   scalar field must emit direct assignment, not memcpy.

   The record has both ARRAY OF CHAR and CARDINAL fields named in a way
   that triggers the array_fields name collision. The critical patterns are:
     items[count].status := local.status    (field-to-field via open array)
     items[count].status := literal         (literal via open array)
     items[count].status := variable        (variable via open array)
     batch[i].inner.status := scalar        (nested record via fixed array)
*)
FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM TokenTypes IMPORT TokenRecord, InitToken;

CONST MaxItems = 4;

TYPE
  Wrapper = RECORD
    inner: TokenRecord;
    seq: CARDINAL;
  END;

VAR
  items: ARRAY [0..3] OF TokenRecord;
  batch: ARRAY [0..1] OF Wrapper;
  local: TokenRecord;
  count: CARDINAL;

PROCEDURE ListItems(VAR arr: ARRAY OF TokenRecord; VAR n: CARDINAL);
VAR
  tmp: TokenRecord;
BEGIN
  (* local.field := literal in procedure with open array param *)
  tmp.status := 0;

  (* open_array[idx].field := local.field — the exact reported pattern *)
  InitToken(tmp, "tok-a", 1);
  arr[n].status := tmp.status;
  INC(n);

  (* open_array[idx].field := literal *)
  arr[n].status := 42;
  INC(n);

  (* open_array[idx].field := variable *)
  tmp.status := 99;
  arr[n].status := tmp.status;
  INC(n)
END ListItems;

BEGIN
  count := 0;
  ListItems(items, count);

  WriteString("count="); WriteCard(count, 1); WriteLn;
  WriteString("s0="); WriteCard(items[0].status, 1); WriteLn;
  WriteString("s1="); WriteCard(items[1].status, 1); WriteLn;
  WriteString("s2="); WriteCard(items[2].status, 1); WriteLn;

  (* Fixed array of nested records: batch[i].inner.status := scalar *)
  InitToken(batch[0].inner, "wrap", 0);
  batch[0].inner.status := 777;
  batch[0].seq := 1;
  WriteString("w0="); WriteCard(batch[0].inner.status, 1); WriteLn;
  WriteString("seq="); WriteCard(batch[0].seq, 1); WriteLn;

  (* Assign from field across nested records *)
  batch[1].inner.status := batch[0].inner.status;
  WriteString("w1="); WriteCard(batch[1].inner.status, 1); WriteLn;

  WriteString("ok"); WriteLn
END OpenArrayScalarMain.
