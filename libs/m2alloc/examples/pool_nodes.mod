MODULE PoolNodes;
(* Fixed-size node pool demo.

   Allocates and frees nodes in a loop, then prints counters.

   Build:
     m2c examples/pool_nodes.mod -I src -o pool_demo
     ./pool_demo

   Expected output:
     allocated 8 nodes
     freed 4 odd nodes
     inUse: 4  highwater: 8  invalidFrees: 0
     re-allocated 4 nodes
     inUse: 8  highwater: 8 *)

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM Pool IMPORT Pool, Init, Alloc, Free, InUse, HighWater, InvalidFrees;

CONST
  BufSize = 1024;
  NodeSize = 48;
  MaxNodes = 16;

VAR
  buf: ARRAY [0..1023] OF CHAR;
  pl: Pool;
  ok: BOOLEAN;
  nodes: ARRAY [0..15] OF ADDRESS;
  i: CARDINAL;

BEGIN
  Init(pl, ADR(buf), BufSize, NodeSize, ok);
  IF NOT ok THEN
    WriteString("pool init failed!"); WriteLn;
    HALT
  END;

  (* Allocate 8 nodes *)
  i := 0;
  WHILE i < 8 DO
    Alloc(pl, nodes[i], ok);
    INC(i)
  END;
  WriteString("allocated 8 nodes"); WriteLn;

  (* Free odd-indexed nodes *)
  i := 1;
  WHILE i < 8 DO
    Free(pl, nodes[i], ok);
    INC(i, 2)
  END;
  WriteString("freed 4 odd nodes"); WriteLn;

  WriteString("inUse: "); WriteCard(InUse(pl), 0);
  WriteString("  highwater: "); WriteCard(HighWater(pl), 0);
  WriteString("  invalidFrees: "); WriteCard(InvalidFrees(pl), 0);
  WriteLn;

  (* Re-allocate 4 nodes into the freed slots *)
  i := 0;
  WHILE i < 4 DO
    Alloc(pl, nodes[8 + i], ok);
    INC(i)
  END;
  WriteString("re-allocated 4 nodes"); WriteLn;

  WriteString("inUse: "); WriteCard(InUse(pl), 0);
  WriteString("  highwater: "); WriteCard(HighWater(pl), 0);
  WriteLn
END PoolNodes.
