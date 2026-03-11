IMPLEMENTATION MODULE AllocUtil;

FROM SYSTEM IMPORT ADDRESS, LONGCARD;

TYPE
  CharPtr = POINTER TO CHAR;

(* ── Alignment ───────────────────────────────────────── *)

PROCEDURE IsPowerOfTwo(x: CARDINAL): BOOLEAN;
VAR v: CARDINAL;
BEGIN
  IF x = 0 THEN RETURN FALSE END;
  v := x;
  WHILE v > 1 DO
    IF (v MOD 2) # 0 THEN RETURN FALSE END;
    v := v DIV 2
  END;
  RETURN TRUE
END IsPowerOfTwo;

PROCEDURE AlignUp(x, align: CARDINAL): CARDINAL;
BEGIN
  IF (align = 0) OR NOT IsPowerOfTwo(align) THEN RETURN x END;
  RETURN ((x + align - 1) DIV align) * align
END AlignUp;

(* ── Pointer arithmetic ──────────────────────────────── *)

PROCEDURE PtrAdd(base: ADDRESS; offset: LONGCARD): ADDRESS;
BEGIN
  RETURN VAL(ADDRESS, LONGCARD(base) + offset)
END PtrAdd;

PROCEDURE PtrDiff(a, b: ADDRESS): LONGCARD;
BEGIN
  IF LONGCARD(b) >= LONGCARD(a) THEN RETURN 0 END;
  RETURN LONGCARD(a) - LONGCARD(b)
END PtrDiff;

(* ── Byte access ─────────────────────────────────────── *)

PROCEDURE FillBytes(base: ADDRESS; count: CARDINAL; val: CARDINAL);
VAR i: CARDINAL; ch: CHAR; p: CharPtr;
BEGIN
  IF count = 0 THEN RETURN END;
  ch := CHR(val MOD 256);
  i := 0;
  WHILE i < count DO
    p := CharPtr(LONGCARD(base) + LONGCARD(i));
    p^ := ch;
    INC(i)
  END
END FillBytes;

PROCEDURE ReadAddr(loc: ADDRESS): ADDRESS;
VAR ap: AddrPtr;
BEGIN
  ap := loc;
  RETURN ap^
END ReadAddr;

PROCEDURE WriteAddr(loc: ADDRESS; val: ADDRESS);
VAR ap: AddrPtr;
BEGIN
  ap := loc;
  ap^ := val
END WriteAddr;

END AllocUtil.
