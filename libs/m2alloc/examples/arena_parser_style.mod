MODULE ArenaParserStyle;
(* Per-request arena demo.

   Simulates a parser that allocates temporaries per "request",
   using mark/reset between requests to reclaim memory.

   Build:
     m2c examples/arena_parser_style.mod -I src -o arena_demo
     ./arena_demo

   Expected output (values depend on stack alignment):
     --- Request 1 ---
     allocated 3 nodes at align 8
     remaining: 736
     highwater: 288
     --- Request 2 ---
     allocated 3 nodes at align 8
     remaining: 736
     highwater: 288
     --- Stats ---
     highwater: 288  failed: 0 *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM Arena IMPORT Arena;

CONST
  BufSize = 1024;
  NodeSize = 96;
  NodeAlign = 8;

VAR
  buf: ARRAY [0..1023] OF CHAR;
  a: Arena;
  p: ADDRESS;
  ok: BOOLEAN;
  m: CARDINAL;
  i, req: CARDINAL;

BEGIN
  Arena.Init(a, ADR(buf), BufSize);

  req := 1;
  WHILE req <= 2 DO
    WriteString("--- Request "); WriteCard(req, 0);
    WriteString(" ---"); WriteLn;

    m := Arena.Mark(a);

    (* Allocate some "nodes" *)
    i := 0;
    WHILE i < 3 DO
      Arena.Alloc(a, NodeSize, NodeAlign, p, ok);
      IF NOT ok THEN
        WriteString("allocation failed!"); WriteLn
      END;
      INC(i)
    END;

    WriteString("allocated 3 nodes at align ");
    WriteCard(NodeAlign, 0); WriteLn;
    WriteString("remaining: "); WriteCard(Arena.Remaining(a), 0); WriteLn;
    WriteString("highwater: "); WriteCard(Arena.HighWater(a), 0); WriteLn;

    Arena.ResetTo(a, m);
    INC(req)
  END;

  WriteString("--- Stats ---"); WriteLn;
  WriteString("highwater: "); WriteCard(Arena.HighWater(a), 0);
  WriteString("  failed: "); WriteCard(Arena.FailedAllocs(a), 0); WriteLn
END ArenaParserStyle.
