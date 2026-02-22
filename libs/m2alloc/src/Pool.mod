IMPLEMENTATION MODULE Pool;

FROM SYSTEM IMPORT ADDRESS, TSIZE;
FROM AllocUtil IMPORT AlignUp, PtrAdd, PtrDiff, FillBytes,
                      ReadAddr, WriteAddr;

(* ── Lifecycle ───────────────────────────────────────── *)

PROCEDURE Init(VAR p: Pool; base: ADDRESS; size: CARDINAL;
               blockSize: CARDINAL; VAR ok: BOOLEAN);
VAR addrSize, bs, count, i, offset: CARDINAL;
    cur, nxt: ADDRESS;
BEGIN
  addrSize := TSIZE(ADDRESS);
  IF blockSize < addrSize THEN
    blockSize := addrSize
  END;
  bs := AlignUp(blockSize, addrSize);
  count := size DIV bs;
  IF count = 0 THEN
    ok := FALSE;
    RETURN
  END;
  p.base := base;
  p.size := size;
  p.blockSize := bs;
  p.blockCount := count;
  p.inUse := 0;
  p.highwater := 0;
  p.invalidFree := 0;
  p.poison := FALSE;

  (* Build free list backwards so first alloc returns lowest address. *)
  p.freeHead := NIL;
  i := count;
  WHILE i > 0 DO
    DEC(i);
    offset := i * bs;
    cur := PtrAdd(base, offset);
    WriteAddr(cur, p.freeHead);
    p.freeHead := cur
  END;
  ok := TRUE
END Init;

(* ── Allocation ──────────────────────────────────────── *)

PROCEDURE Alloc(VAR p: Pool; VAR out: ADDRESS; VAR ok: BOOLEAN);
VAR blk: ADDRESS;
BEGIN
  IF p.freeHead = NIL THEN
    out := NIL;
    ok := FALSE;
    RETURN
  END;
  blk := p.freeHead;
  p.freeHead := ReadAddr(blk);
  IF p.poison THEN
    FillBytes(blk, p.blockSize, 0CDH)
  END;
  INC(p.inUse);
  IF p.inUse > p.highwater THEN
    p.highwater := p.inUse
  END;
  out := blk;
  ok := TRUE
END Alloc;

PROCEDURE Free(VAR p: Pool; addr: ADDRESS; VAR ok: BOOLEAN);
VAR diff, usedSize: CARDINAL;
BEGIN
  IF addr = NIL THEN
    ok := FALSE;
    INC(p.invalidFree);
    RETURN
  END;
  usedSize := p.blockCount * p.blockSize;
  diff := PtrDiff(addr, p.base);
  (* Check: addr must be >= base (diff > 0 or addr = base) and < base+usedSize *)
  IF (VAL(LONGINT, addr) < VAL(LONGINT, p.base)) OR (diff >= usedSize) THEN
    ok := FALSE;
    INC(p.invalidFree);
    RETURN
  END;
  (* Check alignment *)
  IF (diff MOD p.blockSize) # 0 THEN
    ok := FALSE;
    INC(p.invalidFree);
    RETURN
  END;
  IF p.poison THEN
    FillBytes(addr, p.blockSize, 0DDH)
  END;
  WriteAddr(addr, p.freeHead);
  p.freeHead := addr;
  DEC(p.inUse);
  ok := TRUE
END Free;

(* ── Queries ─────────────────────────────────────────── *)

PROCEDURE InUse(VAR p: Pool): CARDINAL;
BEGIN
  RETURN p.inUse
END InUse;

PROCEDURE HighWater(VAR p: Pool): CARDINAL;
BEGIN
  RETURN p.highwater
END HighWater;

PROCEDURE InvalidFrees(VAR p: Pool): CARDINAL;
BEGIN
  RETURN p.invalidFree
END InvalidFrees;

(* ── Poisoning ───────────────────────────────────────── *)

PROCEDURE PoisonOn(VAR p: Pool);
BEGIN
  p.poison := TRUE
END PoisonOn;

PROCEDURE PoisonOff(VAR p: Pool);
BEGIN
  p.poison := FALSE
END PoisonOff;

END Pool.
