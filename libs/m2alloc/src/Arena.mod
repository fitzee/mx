IMPLEMENTATION MODULE Arena;

FROM SYSTEM IMPORT ADDRESS, LONGCARD;
FROM AllocUtil IMPORT AlignUp, IsPowerOfTwo, PtrAdd, FillBytes;

(* ── Lifecycle ───────────────────────────────────────── *)

PROCEDURE Init(VAR a: Arena; base: ADDRESS; size: CARDINAL);
BEGIN
  a.base := base;
  a.size := size;
  a.pos := 0;
  a.highwater := 0;
  a.failed := 0;
  a.poison := FALSE;
  a.overflow := NIL
END Init;

(* ── Allocation ──────────────────────────────────────── *)

PROCEDURE Alloc(VAR a: Arena; n: CARDINAL; align: CARDINAL;
                VAR p: ADDRESS; VAR ok: BOOLEAN);
VAR aligned: CARDINAL;
BEGIN
  IF (align = 0) OR NOT IsPowerOfTwo(align) THEN
    align := 1
  END;
  aligned := AlignUp(a.pos, align);
  IF (aligned > a.size) OR (n > a.size - aligned) THEN
    (* Call overflow handler if set — it may grow the arena *)
    IF a.overflow # NIL THEN
      a.overflow(ADR(a), n);
      (* Retry after handler *)
      aligned := AlignUp(a.pos, align);
      IF (aligned <= a.size) AND (n <= a.size - aligned) THEN
        (* Handler resolved the overflow — fall through to allocate *)
      ELSE
        p := NIL;
        ok := FALSE;
        INC(a.failed);
        RETURN
      END
    ELSE
      p := NIL;
      ok := FALSE;
      INC(a.failed);
      RETURN
    END
  END;
  p := PtrAdd(a.base, LONGCARD(aligned));
  IF a.poison THEN
    FillBytes(p, n, 0CDH)
  END;
  a.pos := aligned + n;
  IF a.pos > a.highwater THEN
    a.highwater := a.pos
  END;
  ok := TRUE
END Alloc;

(* ── Mark / Reset ────────────────────────────────────── *)

PROCEDURE Mark(VAR a: Arena): CARDINAL;
BEGIN
  RETURN a.pos
END Mark;

PROCEDURE ResetTo(VAR a: Arena; mark: CARDINAL);
BEGIN
  IF mark > a.pos THEN RETURN END;
  IF a.poison AND (mark < a.pos) THEN
    FillBytes(PtrAdd(a.base, LONGCARD(mark)), a.pos - mark, 0)
  END;
  a.pos := mark
END ResetTo;

PROCEDURE Clear(VAR a: Arena);
BEGIN
  ResetTo(a, 0)
END Clear;

(* ── Queries ─────────────────────────────────────────── *)

PROCEDURE Remaining(VAR a: Arena): CARDINAL;
BEGIN
  RETURN a.size - a.pos
END Remaining;

PROCEDURE HighWater(VAR a: Arena): CARDINAL;
BEGIN
  RETURN a.highwater
END HighWater;

PROCEDURE FailedAllocs(VAR a: Arena): CARDINAL;
BEGIN
  RETURN a.failed
END FailedAllocs;

(* ── Poisoning ───────────────────────────────────────── *)

PROCEDURE PoisonOn(VAR a: Arena);
BEGIN
  a.poison := TRUE
END PoisonOn;

PROCEDURE PoisonOff(VAR a: Arena);
BEGIN
  a.poison := FALSE
END PoisonOff;

(* ── Overflow handling ─────────────────────────────── *)

PROCEDURE SetOverflowHandler(VAR a: Arena; handler: OverflowProc);
BEGIN
  a.overflow := handler
END SetOverflowHandler;

END Arena.
