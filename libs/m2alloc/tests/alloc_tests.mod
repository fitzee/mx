MODULE AllocTests;
(* Deterministic test suite for m2alloc.

   Tests:
     1.  util.poweroftwo    IsPowerOfTwo for various inputs
     2.  util.alignup       AlignUp for align=1,2,4,8,16
     3.  util.ptradd        PtrAdd/PtrDiff roundtrip
     4.  util.fill          FillBytes fills correct bytes
     5.  util.addr_rw       WriteAddr/ReadAddr roundtrip
     6.  arena.basic        Alloc sequence, pointers correct
     7.  arena.full         Alloc until exhausted
     8.  arena.highwater    HighWater tracks max pos
     9.  arena.mark_reset   Mark, alloc, reset, reuse
    10.  arena.reset_inval  ResetTo with mark > pos is no-op
    11.  arena.clear        Clear resets pos to 0
    12.  arena.align_varied Different alignments in sequence
    13.  arena.poison       Poison alloc/reset patterns
    14.  pool.basic         Init, alloc all, one more fails
    15.  pool.free_realloc  Free and re-alloc (LIFO)
    16.  pool.counters      InUse and HighWater
    17.  pool.invalid_nil   Free(NIL) fails
    18.  pool.invalid_range Free out-of-range fails
    19.  pool.invalid_align Free unaligned fails
    20.  pool.poison        Poison alloc/free patterns
    21.  pool.stress        Deterministic alloc/free mix *)

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteInt, WriteCard;
FROM AllocUtil IMPORT ByteArray, BytePtr, IsPowerOfTwo, AlignUp,
                      PtrAdd, PtrDiff, FillBytes,
                      ReadAddr, WriteAddr;
FROM Arena IMPORT Arena;
FROM Pool IMPORT Pool;

VAR
  passed, failed, total: INTEGER;

PROCEDURE Byte(ch: CHAR): CARDINAL;
(* Read a byte as 0..255, masking sign extension. *)
BEGIN
  RETURN ORD(ch) MOD 256
END Byte;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* ── Test 1: IsPowerOfTwo ─────────────────────────── *)

PROCEDURE TestPowerOfTwo;
BEGIN
  Check("pot: 0 is false",   NOT IsPowerOfTwo(0));
  Check("pot: 1 is true",    IsPowerOfTwo(1));
  Check("pot: 2 is true",    IsPowerOfTwo(2));
  Check("pot: 3 is false",   NOT IsPowerOfTwo(3));
  Check("pot: 4 is true",    IsPowerOfTwo(4));
  Check("pot: 7 is false",   NOT IsPowerOfTwo(7));
  Check("pot: 8 is true",    IsPowerOfTwo(8));
  Check("pot: 16 is true",   IsPowerOfTwo(16));
  Check("pot: 256 is true",  IsPowerOfTwo(256));
  Check("pot: 255 is false", NOT IsPowerOfTwo(255))
END TestPowerOfTwo;

(* ── Test 2: AlignUp ─────────────────────────────── *)

PROCEDURE TestAlignUp;
BEGIN
  Check("align: 0 a=1 -> 0",   AlignUp(0, 1) = 0);
  Check("align: 1 a=1 -> 1",   AlignUp(1, 1) = 1);
  Check("align: 5 a=1 -> 5",   AlignUp(5, 1) = 5);
  Check("align: 0 a=4 -> 0",   AlignUp(0, 4) = 0);
  Check("align: 1 a=4 -> 4",   AlignUp(1, 4) = 4);
  Check("align: 3 a=4 -> 4",   AlignUp(3, 4) = 4);
  Check("align: 4 a=4 -> 4",   AlignUp(4, 4) = 4);
  Check("align: 5 a=4 -> 8",   AlignUp(5, 4) = 8);
  Check("align: 7 a=8 -> 8",   AlignUp(7, 8) = 8);
  Check("align: 8 a=8 -> 8",   AlignUp(8, 8) = 8);
  Check("align: 9 a=8 -> 16",  AlignUp(9, 8) = 16);
  Check("align: 1 a=16 -> 16", AlignUp(1, 16) = 16);
  Check("align: 16 a=16 -> 16", AlignUp(16, 16) = 16);
  Check("align: 17 a=16 -> 32", AlignUp(17, 16) = 32);
  (* non-power-of-two align returns x unchanged *)
  Check("align: 5 a=3 -> 5",   AlignUp(5, 3) = 5);
  Check("align: 5 a=0 -> 5",   AlignUp(5, 0) = 5)
END TestAlignUp;

(* ── Test 3: PtrAdd / PtrDiff ────────────────────── *)

PROCEDURE TestPtrAddDiff;
VAR buf: ARRAY [0..255] OF CHAR;
    base, p: ADDRESS;
BEGIN
  base := ADR(buf);
  p := PtrAdd(base, 10);
  Check("ptradd: diff=10", PtrDiff(p, base) = 10);
  p := PtrAdd(base, 0);
  Check("ptradd: diff=0", PtrDiff(p, base) = 0);
  p := PtrAdd(base, 100);
  Check("ptradd: diff=100", PtrDiff(p, base) = 100);
  (* PtrDiff returns 0 when b >= a *)
  Check("ptrdiff: b>=a -> 0", PtrDiff(base, p) = 0)
END TestPtrAddDiff;

(* ── Test 4: FillBytes ───────────────────────────── *)

PROCEDURE TestFillBytes;
VAR buf: ARRAY [0..15] OF CHAR;
    bp: BytePtr;
BEGIN
  (* Clear buffer *)
  FillBytes(ADR(buf), 16, 0);
  bp := ADR(buf);
  Check("fill: initial 0", Byte(bp^[0]) = 0);
  Check("fill: initial 15", Byte(bp^[15]) = 0);
  (* Fill with 0xAB *)
  FillBytes(ADR(buf), 16, 0ABH);
  Check("fill: 0xAB at 0", Byte(bp^[0]) = 0ABH);
  Check("fill: 0xAB at 7", Byte(bp^[7]) = 0ABH);
  Check("fill: 0xAB at 15", Byte(bp^[15]) = 0ABH);
  (* Partial fill *)
  FillBytes(ADR(buf), 4, 0FFH);
  Check("fill: partial 0xFF at 0", Byte(bp^[0]) = 0FFH);
  Check("fill: partial 0xFF at 3", Byte(bp^[3]) = 0FFH);
  Check("fill: untouched at 4", Byte(bp^[4]) = 0ABH)
END TestFillBytes;

(* ── Test 5: ReadAddr / WriteAddr ────────────────── *)

PROCEDURE TestAddrRW;
VAR buf: ARRAY [0..31] OF CHAR;
    target: ADDRESS;
    result: ADDRESS;
BEGIN
  target := ADR(buf);
  WriteAddr(ADR(buf), target);
  result := ReadAddr(ADR(buf));
  Check("addr_rw: roundtrip", result = target);
  WriteAddr(ADR(buf), NIL);
  result := ReadAddr(ADR(buf));
  Check("addr_rw: nil roundtrip", result = NIL)
END TestAddrRW;

(* ── Test 6: Arena basic ─────────────────────────── *)

PROCEDURE TestArenaBasic;
VAR buf: ARRAY [0..1023] OF CHAR;
    a: Arena;
    p1, p2, p3: ADDRESS;
    ok: BOOLEAN;
BEGIN
  Arena.Init(a, ADR(buf), 1024);
  Check("arena: remaining=1024", Arena.Remaining(a) = 1024);
  Check("arena: highwater=0", Arena.HighWater(a) = 0);

  Arena.Alloc(a, 32, 1, p1, ok);
  Check("arena: alloc1 ok", ok);
  Check("arena: p1 not nil", p1 # NIL);

  Arena.Alloc(a, 64, 1, p2, ok);
  Check("arena: alloc2 ok", ok);
  Check("arena: p2 > p1", VAL(LONGINT, p2) > VAL(LONGINT, p1));

  Arena.Alloc(a, 16, 1, p3, ok);
  Check("arena: alloc3 ok", ok);
  Check("arena: p3 > p2", VAL(LONGINT, p3) > VAL(LONGINT, p2));

  Check("arena: remaining decreased", Arena.Remaining(a) < 1024);
  Check("arena: remaining=912", Arena.Remaining(a) = 1024 - 32 - 64 - 16)
END TestArenaBasic;

(* ── Test 7: Arena full ──────────────────────────── *)

PROCEDURE TestArenaFull;
VAR buf: ARRAY [0..63] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
BEGIN
  Arena.Init(a, ADR(buf), 64);

  Arena.Alloc(a, 64, 1, p, ok);
  Check("arena.full: alloc 64 ok", ok);
  Check("arena.full: remaining=0", Arena.Remaining(a) = 0);

  Arena.Alloc(a, 1, 1, p, ok);
  Check("arena.full: alloc 1 fails", NOT ok);
  Check("arena.full: p=NIL", p = NIL);
  Check("arena.full: failed=1", Arena.FailedAllocs(a) = 1);

  Arena.Alloc(a, 1, 1, p, ok);
  Check("arena.full: failed=2", Arena.FailedAllocs(a) = 2)
END TestArenaFull;

(* ── Test 8: Arena highwater ─────────────────────── *)

PROCEDURE TestArenaHighwater;
VAR buf: ARRAY [0..255] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
    m: CARDINAL;
BEGIN
  Arena.Init(a, ADR(buf), 256);

  Arena.Alloc(a, 100, 1, p, ok);
  Check("arena.hw: after 100", Arena.HighWater(a) = 100);

  m := Arena.Mark(a);
  Arena.Alloc(a, 50, 1, p, ok);
  Check("arena.hw: after 150", Arena.HighWater(a) = 150);

  Arena.ResetTo(a, m);
  Check("arena.hw: after reset still 150", Arena.HighWater(a) = 150);

  Arena.Alloc(a, 10, 1, p, ok);
  Check("arena.hw: still 150", Arena.HighWater(a) = 150)
END TestArenaHighwater;

(* ── Test 9: Arena mark/reset ────────────────────── *)

PROCEDURE TestArenaMarkReset;
VAR buf: ARRAY [0..255] OF CHAR;
    a: Arena;
    p1, p2: ADDRESS;
    ok: BOOLEAN;
    m: CARDINAL;
BEGIN
  Arena.Init(a, ADR(buf), 256);

  Arena.Alloc(a, 32, 1, p1, ok);
  m := Arena.Mark(a);

  Arena.Alloc(a, 64, 1, p2, ok);
  Check("arena.mr: p2 ok", ok);

  Arena.ResetTo(a, m);
  Check("arena.mr: pos reset", Arena.Remaining(a) = 256 - 32);

  (* Alloc again should reuse same address *)
  Arena.Alloc(a, 64, 1, p2, ok);
  Check("arena.mr: reuses addr", p2 = PtrAdd(ADR(buf), 32))
END TestArenaMarkReset;

(* ── Test 10: Arena reset invalid ────────────────── *)

PROCEDURE TestArenaResetInvalid;
VAR buf: ARRAY [0..127] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
BEGIN
  Arena.Init(a, ADR(buf), 128);
  Arena.Alloc(a, 32, 1, p, ok);
  Arena.ResetTo(a, 100);  (* mark > pos: no-op *)
  Check("arena.ri: pos unchanged", Arena.Remaining(a) = 128 - 32)
END TestArenaResetInvalid;

(* ── Test 11: Arena clear ────────────────────────── *)

PROCEDURE TestArenaClear;
VAR buf: ARRAY [0..127] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
BEGIN
  Arena.Init(a, ADR(buf), 128);
  Arena.Alloc(a, 50, 1, p, ok);
  Arena.Clear(a);
  Check("arena.clear: remaining=128", Arena.Remaining(a) = 128)
END TestArenaClear;

(* ── Test 12: Arena varied alignment ─────────────── *)

PROCEDURE TestArenaAlignVaried;
VAR buf: ARRAY [0..1023] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
BEGIN
  Arena.Init(a, ADR(buf), 1024);

  (* Alloc 1 byte unaligned to push pos off alignment *)
  Arena.Alloc(a, 1, 1, p, ok);
  Check("arena.av: 1 byte ok", ok);

  (* Alloc with align=4 *)
  Arena.Alloc(a, 8, 4, p, ok);
  Check("arena.av: align4 ok", ok);
  Check("arena.av: align4 mod", (PtrDiff(p, ADR(buf)) MOD 4) = 0);

  (* Alloc 1 byte to push off again *)
  Arena.Alloc(a, 1, 1, p, ok);

  (* Alloc with align=8 *)
  Arena.Alloc(a, 16, 8, p, ok);
  Check("arena.av: align8 ok", ok);
  Check("arena.av: align8 mod", (PtrDiff(p, ADR(buf)) MOD 8) = 0);

  (* Alloc with align=16 *)
  Arena.Alloc(a, 32, 16, p, ok);
  Check("arena.av: align16 ok", ok);
  Check("arena.av: align16 mod", (PtrDiff(p, ADR(buf)) MOD 16) = 0)
END TestArenaAlignVaried;

(* ── Test 13: Arena poison ───────────────────────── *)

PROCEDURE TestArenaPoison;
VAR buf: ARRAY [0..255] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
    bp: BytePtr;
    m: CARDINAL;
BEGIN
  Arena.Init(a, ADR(buf), 256);
  Arena.PoisonOn(a);

  Arena.Alloc(a, 8, 1, p, ok);
  Check("arena.poison: alloc ok", ok);
  bp := p;
  Check("arena.poison: 0xCD at 0", Byte(bp^[0]) = 0CDH);
  Check("arena.poison: 0xCD at 7", Byte(bp^[7]) = 0CDH);

  m := Arena.Mark(a);
  Arena.Alloc(a, 4, 1, p, ok);
  Arena.ResetTo(a, m);
  bp := PtrAdd(ADR(buf), m);
  Check("arena.poison: reset 0 at 0", Byte(bp^[0]) = 0);
  Check("arena.poison: reset 0 at 3", Byte(bp^[3]) = 0)
END TestArenaPoison;

(* ── Test 14: Pool basic ─────────────────────────── *)

PROCEDURE TestPoolBasic;
VAR buf: ARRAY [0..511] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
    blk: ADDRESS;
    i, count: CARDINAL;
BEGIN
  Pool.Init(pl, ADR(buf), 512, 32, ok);
  Check("pool.basic: init ok", ok);
  count := 512 DIV 32;
  Check("pool.basic: blockCount=16", pl.blockCount = count);

  (* Alloc all blocks *)
  i := 0;
  WHILE i < count DO
    Pool.Alloc(pl, blk, ok);
    Check("pool.basic: alloc ok", ok);
    INC(i)
  END;

  (* One more should fail *)
  Pool.Alloc(pl, blk, ok);
  Check("pool.basic: exhausted", NOT ok);
  Check("pool.basic: blk=NIL", blk = NIL)
END TestPoolBasic;

(* ── Test 15: Pool free/realloc LIFO ─────────────── *)

PROCEDURE TestPoolFreeRealloc;
VAR buf: ARRAY [0..255] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
    a1, a2, a3, r1, r2: ADDRESS;
BEGIN
  Pool.Init(pl, ADR(buf), 256, 32, ok);

  Pool.Alloc(pl, a1, ok);
  Pool.Alloc(pl, a2, ok);
  Pool.Alloc(pl, a3, ok);

  (* Free a2 then a1 *)
  Pool.Free(pl, a2, ok);
  Check("pool.fr: free a2 ok", ok);
  Pool.Free(pl, a1, ok);
  Check("pool.fr: free a1 ok", ok);

  (* Realloc: LIFO -> a1 first, then a2 *)
  Pool.Alloc(pl, r1, ok);
  Check("pool.fr: realloc r1=a1", r1 = a1);
  Pool.Alloc(pl, r2, ok);
  Check("pool.fr: realloc r2=a2", r2 = a2)
END TestPoolFreeRealloc;

(* ── Test 16: Pool counters ──────────────────────── *)

PROCEDURE TestPoolCounters;
VAR buf: ARRAY [0..255] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
    a1, a2, a3: ADDRESS;
BEGIN
  Pool.Init(pl, ADR(buf), 256, 32, ok);

  Pool.Alloc(pl, a1, ok);
  Pool.Alloc(pl, a2, ok);
  Pool.Alloc(pl, a3, ok);
  Check("pool.ctr: inUse=3", Pool.InUse(pl) = 3);
  Check("pool.ctr: hw=3", Pool.HighWater(pl) = 3);

  Pool.Free(pl, a2, ok);
  Check("pool.ctr: inUse=2", Pool.InUse(pl) = 2);
  Check("pool.ctr: hw still 3", Pool.HighWater(pl) = 3)
END TestPoolCounters;

(* ── Test 17: Pool invalid nil ───────────────────── *)

PROCEDURE TestPoolInvalidNil;
VAR buf: ARRAY [0..127] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
BEGIN
  Pool.Init(pl, ADR(buf), 128, 16, ok);
  Pool.Free(pl, NIL, ok);
  Check("pool.nil: ok=FALSE", NOT ok);
  Check("pool.nil: invalidFree=1", Pool.InvalidFrees(pl) = 1)
END TestPoolInvalidNil;

(* ── Test 18: Pool invalid range ─────────────────── *)

PROCEDURE TestPoolInvalidRange;
VAR buf: ARRAY [0..127] OF CHAR;
    other: ARRAY [0..31] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
BEGIN
  Pool.Init(pl, ADR(buf), 128, 16, ok);
  Pool.Free(pl, ADR(other), ok);
  Check("pool.range: ok=FALSE", NOT ok);
  Check("pool.range: invalidFree=1", Pool.InvalidFrees(pl) = 1)
END TestPoolInvalidRange;

(* ── Test 19: Pool invalid alignment ─────────────── *)

PROCEDURE TestPoolInvalidAlign;
VAR buf: ARRAY [0..127] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
    unaligned: ADDRESS;
BEGIN
  Pool.Init(pl, ADR(buf), 128, 16, ok);
  (* Create an unaligned address within the pool *)
  unaligned := PtrAdd(ADR(buf), 3);
  Pool.Free(pl, unaligned, ok);
  Check("pool.align: ok=FALSE", NOT ok);
  Check("pool.align: invalidFree=1", Pool.InvalidFrees(pl) = 1)
END TestPoolInvalidAlign;

(* ── Test 20: Pool poison ────────────────────────── *)

PROCEDURE TestPoolPoison;
VAR buf: ARRAY [0..255] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
    blk: ADDRESS;
    bp: BytePtr;
BEGIN
  Pool.Init(pl, ADR(buf), 256, 32, ok);
  Pool.PoisonOn(pl);

  Pool.Alloc(pl, blk, ok);
  Check("pool.poison: alloc ok", ok);
  bp := blk;
  Check("pool.poison: 0xCD at 0", Byte(bp^[0]) = 0CDH);
  Check("pool.poison: 0xCD at 31", Byte(bp^[31]) = 0CDH);

  Pool.Free(pl, blk, ok);
  Check("pool.poison: free ok", ok);
  bp := blk;
  (* First 8 bytes hold next pointer on 64-bit, check beyond that *)
  Check("pool.poison: 0xDD at 8", Byte(bp^[8]) = 0DDH);
  Check("pool.poison: 0xDD at 31", Byte(bp^[31]) = 0DDH)
END TestPoolPoison;

(* ── Test 21: Pool stress ────────────────────────── *)

PROCEDURE TestPoolStress;
VAR buf: ARRAY [0..2047] OF CHAR;
    pl: Pool;
    ok: BOOLEAN;
    ptrs: ARRAY [0..63] OF ADDRESS;
    held: ARRAY [0..63] OF BOOLEAN;
    count, i, rng, idx: CARDINAL;
BEGIN
  Pool.Init(pl, ADR(buf), 2048, 32, ok);
  Check("pool.stress: init ok", ok);
  count := 2048 DIV 32;

  (* Clear ptrs *)
  i := 0;
  WHILE i < 64 DO
    ptrs[i] := NIL;
    held[i] := FALSE;
    INC(i)
  END;

  (* LCG-based deterministic alloc/free for 200 rounds *)
  rng := 12345;
  i := 0;
  WHILE i < 200 DO
    rng := (rng * 1103515245 + 12345) MOD 65536;
    idx := rng MOD count;
    IF held[idx] THEN
      Pool.Free(pl, ptrs[idx], ok);
      held[idx] := FALSE
    ELSE
      Pool.Alloc(pl, ptrs[idx], ok);
      IF ok THEN
        held[idx] := TRUE
      END
    END;
    INC(i)
  END;

  Check("pool.stress: inUse <= count", Pool.InUse(pl) <= count);
  Check("pool.stress: hw <= count", Pool.HighWater(pl) <= count);
  Check("pool.stress: invalidFree=0", Pool.InvalidFrees(pl) = 0)
END TestPoolStress;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestPowerOfTwo;
  TestAlignUp;
  TestPtrAddDiff;
  TestFillBytes;
  TestAddrRW;
  TestArenaBasic;
  TestArenaFull;
  TestArenaHighwater;
  TestArenaMarkReset;
  TestArenaResetInvalid;
  TestArenaClear;
  TestArenaAlignVaried;
  TestArenaPoison;
  TestPoolBasic;
  TestPoolFreeRealloc;
  TestPoolCounters;
  TestPoolInvalidNil;
  TestPoolInvalidRange;
  TestPoolInvalidAlign;
  TestPoolPoison;
  TestPoolStress;

  WriteLn;
  WriteString("m2alloc: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END AllocTests.
