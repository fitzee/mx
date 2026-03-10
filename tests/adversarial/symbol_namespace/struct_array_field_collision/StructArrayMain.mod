MODULE StructArrayMain;
(* Regression test: when two imported records have a field with the same name
   but different types (ARRAY OF CHAR vs RECORD), assignment to the RECORD-typed
   field must emit direct struct assignment, not memcpy.

   ArrayBody.Packet.body  = ARRAY [0..63] OF CHAR
   StructBody.Msg.body    = Payload (RECORD)

   If the codegen uses name-only is_array_field("body"), it will incorrectly
   route Msg.body assignments through memcpy, causing C type errors. *)

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM ArrayBody IMPORT Packet, InitPacket;
FROM StructBody IMPORT Msg, Payload, InitMsg;

VAR
  pkt: Packet;
  m1, m2: Msg;
  p: Payload;

BEGIN
  (* Array-typed body: should use memcpy/strcpy *)
  InitPacket(pkt, "hello", 5);
  WriteString("pkt="); WriteString(pkt.body); WriteLn;

  (* Struct-typed body: must use direct assignment *)
  InitMsg(m1, 10, 200, 1);
  WriteString("t1="); WriteCard(m1.body.tag, 1); WriteLn;
  WriteString("s1="); WriteCard(m1.body.size, 1); WriteLn;
  WriteString("q1="); WriteCard(m1.seq, 1); WriteLn;

  (* Struct-to-struct field copy: m2.body := m1.body *)
  InitMsg(m2, 0, 0, 2);
  m2.body := m1.body;
  WriteString("t2="); WriteCard(m2.body.tag, 1); WriteLn;
  WriteString("s2="); WriteCard(m2.body.size, 1); WriteLn;
  WriteString("q2="); WriteCard(m2.seq, 1); WriteLn;

  (* Assign from local record variable *)
  p.tag := 99;
  p.size := 512;
  m2.body := p;
  WriteString("t3="); WriteCard(m2.body.tag, 1); WriteLn;
  WriteString("s3="); WriteCard(m2.body.size, 1); WriteLn;

  WriteString("ok"); WriteLn
END StructArrayMain.
