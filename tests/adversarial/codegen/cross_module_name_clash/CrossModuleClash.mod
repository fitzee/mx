MODULE CrossModuleClash;
(* Adversarial test: cross-module name clashes + re-exported enums.
   Tests:
   1. Two modules (NetA, NetB) define impl-only types with the SAME
      names (ConnRec, ConnPtr) but DIFFERENT fields
   2. Nested deref chain: c^.resp^.headerCount across 3 type boundaries
   3. Re-exported enum: NetB re-exports NetA.Status; accessing NetB.Ok
      must resolve to the correct enum variant value
   4. INC on deeply nested field through pointer chain
   5. VAR record param with field access *)

FROM NetA IMPORT Response, ResponsePtr, MakeResponse, Status, Ok, Error;
FROM NetB IMPORT Endpoint, Connect;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Strings IMPORT Assign;
IMPORT NetB;

VAR
  resp: ResponsePtr;
  ep: Endpoint;
  st: Status;

BEGIN
  (* Test 1: create response, verify code *)
  resp := MakeResponse(200);
  WriteString("code=");
  WriteInt(resp^.code, 0);
  WriteLn;

  (* Test 2: INC on nested field through pointer *)
  INC(resp^.headerCount);
  INC(resp^.headerCount);
  WriteString("hcount=");
  WriteInt(resp^.headerCount, 0);
  WriteLn;

  (* Test 3: re-exported enum via NetB *)
  st := NetB.Ok;
  WriteString("ok=");
  WriteInt(ORD(st), 0);
  WriteLn;

  st := NetB.Error;
  WriteString("err=");
  WriteInt(ORD(st), 0);
  WriteLn;

  (* Test 4: VAR record param with enum field *)
  Assign("localhost", ep.host);
  ep.port := 8080;
  st := Connect(ep);
  WriteString("conn=");
  WriteInt(ORD(st), 0);
  WriteLn;
  WriteString("epst=");
  WriteInt(ORD(ep.status), 0);
  WriteLn
END CrossModuleClash.
