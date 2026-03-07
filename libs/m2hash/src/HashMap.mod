IMPLEMENTATION MODULE HashMap;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Strings IMPORT Length;

(* ── Pointer arithmetic for bucket access ────────────── *)
(* Avoids POINTER TO ARRAY OF Record overlay which has a
   codegen issue.  Uses single-element pointer + arithmetic. *)

TYPE
  BucketPtr = POINTER TO Bucket;

PROCEDURE BucketAt(base: ADDRESS; idx: CARDINAL): BucketPtr;
BEGIN
  RETURN VAL(ADDRESS,
    VAL(LONGINT, base) + VAL(LONGINT, idx) * VAL(LONGINT, TSIZE(Bucket)))
END BucketAt;

(* ── FNV-1a constants ────────────────────────────────── *)

CONST
  FNVOffset = 2166136261;
  FNVPrime  = 16777619;

(* ── Internal helpers ────────────────────────────────── *)

PROCEDURE StrEq(VAR a: ARRAY OF CHAR; VAR b: ARRAY OF CHAR): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(a)) AND (i <= HIGH(b)) DO
    IF a[i] # b[i] THEN RETURN FALSE END;
    IF a[i] = 0C THEN RETURN TRUE END;
    INC(i)
  END;
  IF (i <= HIGH(a)) AND (a[i] # 0C) THEN RETURN FALSE END;
  IF (i <= HIGH(b)) AND (b[i] # 0C) THEN RETURN FALSE END;
  RETURN TRUE
END StrEq;

PROCEDURE CopyStr(VAR src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(src)) AND (i <= HIGH(dst)) DO
    dst[i] := src[i];
    IF src[i] = 0C THEN RETURN END;
    INC(i)
  END;
  IF i <= HIGH(dst) THEN
    dst[i] := 0C
  END
END CopyStr;

(* ── Hash function ───────────────────────────────────── *)

PROCEDURE Hash(key: ARRAY OF CHAR): CARDINAL;
VAR
  h, i: CARDINAL;
BEGIN
  h := FNVOffset;
  i := 0;
  WHILE (i <= HIGH(key)) AND (key[i] # 0C) DO
    h := BXOR(h, ORD(key[i]) MOD 256);
    h := BAND(h * FNVPrime, 0FFFFFFFFh);
    INC(i)
  END;
  RETURN h
END Hash;

(* ── FindSlot: locate a key or first available slot ──── *)

PROCEDURE FindSlot(VAR m: Map; VAR key: ARRAY OF CHAR;
                   VAR idx: CARDINAL; VAR found: BOOLEAN);
VAR
  bp: BucketPtr;
  start, i, tombstone: CARDINAL;
  haveTomb: BOOLEAN;
BEGIN
  found := FALSE;
  start := Hash(key) MOD m.cap;
  haveTomb := FALSE;
  tombstone := 0;
  i := 0;
  WHILE i < m.cap DO
    idx := (start + i) MOD m.cap;
    bp := BucketAt(m.base, idx);
    IF bp^.occupied THEN
      IF bp^.deleted THEN
        IF NOT haveTomb THEN
          tombstone := idx;
          haveTomb := TRUE
        END
      ELSE
        IF StrEq(bp^.key, key) THEN
          found := TRUE;
          RETURN
        END
      END
    ELSE
      IF haveTomb THEN
        idx := tombstone
      END;
      RETURN
    END;
    INC(i)
  END;
  IF haveTomb THEN
    idx := tombstone
  ELSE
    idx := m.cap
  END
END FindSlot;

(* ── Public API ──────────────────────────────────────── *)

PROCEDURE Init(VAR m: Map; buckets: ADDRESS; cap: CARDINAL);
VAR bp: BucketPtr; i: CARDINAL;
BEGIN
  m.base := buckets;
  m.cap := cap;
  m.count := 0;
  i := 0;
  WHILE i < cap DO
    bp := BucketAt(buckets, i);
    bp^.key[0] := 0C;
    bp^.val := 0;
    bp^.occupied := FALSE;
    bp^.deleted := FALSE;
    INC(i)
  END
END Init;

PROCEDURE Clear(VAR m: Map);
VAR bp: BucketPtr; i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < m.cap DO
    bp := BucketAt(m.base, i);
    bp^.key[0] := 0C;
    bp^.val := 0;
    bp^.occupied := FALSE;
    bp^.deleted := FALSE;
    INC(i)
  END;
  m.count := 0
END Clear;

PROCEDURE Put(VAR m: Map; key: ARRAY OF CHAR; val: INTEGER): BOOLEAN;
VAR bp: BucketPtr; idx: CARDINAL; found: BOOLEAN;
BEGIN
  FindSlot(m, key, idx, found);
  IF found THEN
    bp := BucketAt(m.base, idx);
    bp^.val := val;
    RETURN TRUE
  END;
  IF idx >= m.cap THEN RETURN FALSE END;
  bp := BucketAt(m.base, idx);
  CopyStr(key, bp^.key);
  bp^.val := val;
  bp^.occupied := TRUE;
  bp^.deleted := FALSE;
  INC(m.count);
  RETURN TRUE
END Put;

PROCEDURE Get(VAR m: Map; key: ARRAY OF CHAR; VAR val: INTEGER): BOOLEAN;
VAR bp: BucketPtr; idx: CARDINAL; found: BOOLEAN;
BEGIN
  FindSlot(m, key, idx, found);
  IF found THEN
    bp := BucketAt(m.base, idx);
    val := bp^.val;
    RETURN TRUE
  END;
  RETURN FALSE
END Get;

PROCEDURE Contains(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN;
VAR idx: CARDINAL; found: BOOLEAN;
BEGIN
  FindSlot(m, key, idx, found);
  RETURN found
END Contains;

PROCEDURE Remove(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN;
VAR bp: BucketPtr; idx: CARDINAL; found: BOOLEAN;
BEGIN
  FindSlot(m, key, idx, found);
  IF NOT found THEN RETURN FALSE END;
  bp := BucketAt(m.base, idx);
  bp^.deleted := TRUE;
  DEC(m.count);
  RETURN TRUE
END Remove;

PROCEDURE Count(VAR m: Map): CARDINAL;
BEGIN
  RETURN m.count
END Count;

END HashMap.
