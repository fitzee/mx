IMPLEMENTATION MODULE Fsm;
(* Cache-optimised, PIM4-compliant FSM core.
 *
 * Transition record layout (CARDINAL = uint32_t on m2c):
 *   Offset 0: next   (4 bytes, align 4)
 *   Offset 4: action (4 bytes, align 4)
 *   Offset 8: guard  (4 bytes, align 4)
 *   Stride:   12 bytes  (multiple of 4 — no padding needed)
 *
 * Every element in a contiguous ARRAY OF Transition is naturally
 * aligned when addressed by base + idx * TSIZE(Transition).
 * Cache density: 5 transitions per 64-byte line.
 *
 * Pointer arithmetic uses LONGCARD (uint64_t) type transfers
 * so addresses are never truncated on ARM64 (M4 / Graviton).
 * No ISO ADDADR, no hardcoded array overlays.
 *)

FROM SYSTEM IMPORT ADDRESS, TSIZE;

(* ── Pointer-to-element types ───────────────────────── *)
(* One pointer type per element kind.  Arithmetic is done in
   LONGCARD; the result is type-transferred back.             *)

TYPE
  TransPtr = POINTER TO Transition;
  ActPtr   = POINTER TO ActionProc;
  GrdPtr   = POINTER TO GuardProc;
  HookPtr  = POINTER TO HookProc;

(* ── Internal helpers ────────────────────────────────── *)

PROCEDURE DoTrace(VAR f: Fsm; from, to: StateId;
                  ev: EventId; act: ActionId; st: StepStatus);
VAR tr: TraceProc;
BEGIN
  tr := f.trace;
  IF tr # NIL THEN
    tr(f.traceCtx, from, to, ev, act, st)
  END
END DoTrace;

(* ── Lifecycle ───────────────────────────────────────── *)

PROCEDURE Init(VAR f: Fsm; start: StateId; ctx: ADDRESS;
               numStates, numEvents: CARDINAL; trans: ADDRESS);
BEGIN
  f.state := start;
  f.start := start;
  f.ctx := ctx;
  f.trans := trans;
  f.numStates := numStates;
  f.numEvents := numEvents;
  f.acts := NIL;
  f.numActs := 0;
  f.guards := NIL;
  f.numGuards := 0;
  f.onEnter := NIL;
  f.onExit := NIL;
  f.trace := NIL;
  f.traceCtx := NIL;
  f.steps := 0;
  f.invalid := 0;
  f.rejected := 0;
  f.errors := 0
END Init;

PROCEDURE Reset(VAR f: Fsm);
BEGIN
  f.state := f.start;
  f.steps := 0;
  f.invalid := 0;
  f.rejected := 0;
  f.errors := 0
END Reset;

(* ── Configuration ───────────────────────────────────── *)

PROCEDURE SetActions(VAR f: Fsm; acts: ADDRESS; numActs: CARDINAL);
BEGIN
  f.acts := acts;
  f.numActs := numActs
END SetActions;

PROCEDURE SetGuards(VAR f: Fsm; guards: ADDRESS; numGuards: CARDINAL);
BEGIN
  f.guards := guards;
  f.numGuards := numGuards
END SetGuards;

PROCEDURE SetHooks(VAR f: Fsm; enterHooks, exitHooks: ADDRESS);
BEGIN
  f.onEnter := enterHooks;
  f.onExit := exitHooks
END SetHooks;

PROCEDURE SetTrace(VAR f: Fsm; proc: TraceProc; traceCtx: ADDRESS);
BEGIN
  f.trace := proc;
  f.traceCtx := traceCtx
END SetTrace;

(* ── Core ────────────────────────────────────────────── *)
(* Fast-path layout:
 *   1. Bounds check          (cold — branch predictor learns quickly)
 *   2. Table lookup          (one multiply + pointer add)
 *   3. NoTransition check    (single compare against sentinel)
 *   4. Guard check           (skipped entirely when grdId = 0)
 *   5. Exit hook / transition / Enter hook
 *   6. Action
 *
 * NoTransition and GuardRejected exit before any state mutation,
 * keeping the hot path (Ok) as a straight-line fall-through.     *)

PROCEDURE Step(VAR f: Fsm; ev: EventId; payload: ADDRESS;
               VAR status: StepStatus);
VAR
  tp: TransPtr;
  ap: ActPtr;
  gp: GrdPtr;
  hp: HookPtr;
  act: ActionProc;
  grd: GuardProc;
  hook: HookProc;
  fromState, nextState: StateId;
  actId: ActionId;
  grdId: GuardId;
  idx: CARDINAL;
  allow, ok: BOOLEAN;
BEGIN
  fromState := f.state;

  (* 1. Bounds check — cold path *)
  IF (fromState >= f.numStates) OR (ev >= f.numEvents) THEN
    status := Error;
    INC(f.errors);
    DoTrace(f, fromState, fromState, ev, NoAction, Error);
    RETURN
  END;

  (* 2. Table lookup — O(1), one pointer add *)
  idx := fromState * f.numEvents + ev;
  tp := TransPtr(LONGCARD(f.trans)
        + LONGCARD(idx * TSIZE(Transition)));
  nextState := tp^.next;

  (* 3. NoTransition — fast reject before any work *)
  IF nextState = NoState THEN
    status := NoTransition;
    INC(f.invalid);
    DoTrace(f, fromState, fromState, ev, NoAction, NoTransition);
    RETURN
  END;

  (* Load action/guard from the same cache line as next *)
  actId := tp^.action;
  grdId := tp^.guard;

  (* 4. Guard — single sentinel check gates the entire block *)
  IF grdId # NoGuard THEN
    IF (f.guards # NIL) AND (grdId < f.numGuards) THEN
      gp := GrdPtr(LONGCARD(f.guards)
            + LONGCARD(grdId * TSIZE(GuardProc)));
      grd := gp^;
      IF grd # NIL THEN
        allow := TRUE;
        grd(f.ctx, ev, payload, allow);
        IF NOT allow THEN
          status := GuardRejected;
          INC(f.rejected);
          DoTrace(f, fromState, fromState, ev, actId, GuardRejected);
          RETURN
        END
      END
    END
  END;

  (* 5a. Exit hook — fromState already bounds-checked *)
  IF f.onExit # NIL THEN
    hp := HookPtr(LONGCARD(f.onExit)
          + LONGCARD(fromState * TSIZE(HookProc)));
    hook := hp^;
    IF hook # NIL THEN
      hook(f.ctx, fromState, payload)
    END
  END;

  (* 5b. State transition *)
  f.state := nextState;

  (* 5c. Enter hook — bounds-check nextState (table could be wrong) *)
  IF (f.onEnter # NIL) AND (nextState < f.numStates) THEN
    hp := HookPtr(LONGCARD(f.onEnter)
          + LONGCARD(nextState * TSIZE(HookProc)));
    hook := hp^;
    IF hook # NIL THEN
      hook(f.ctx, nextState, payload)
    END
  END;

  (* 6. Action *)
  IF actId # NoAction THEN
    IF (f.acts # NIL) AND (actId < f.numActs) THEN
      ap := ActPtr(LONGCARD(f.acts)
            + LONGCARD(actId * TSIZE(ActionProc)));
      act := ap^;
      IF act # NIL THEN
        ok := TRUE;
        act(f.ctx, ev, payload, ok);
        IF NOT ok THEN
          status := Error;
          INC(f.errors);
          DoTrace(f, fromState, nextState, ev, actId, Error);
          RETURN
        END
      END
    END
  END;

  status := Ok;
  INC(f.steps);
  DoTrace(f, fromState, nextState, ev, actId, Ok)
END Step;

(* ── Queries ─────────────────────────────────────────── *)

PROCEDURE CurrentState(VAR f: Fsm): StateId;
BEGIN
  RETURN f.state
END CurrentState;

PROCEDURE StepCount(VAR f: Fsm): CARDINAL;
BEGIN
  RETURN f.steps
END StepCount;

PROCEDURE InvalidCount(VAR f: Fsm): CARDINAL;
BEGIN
  RETURN f.invalid
END InvalidCount;

PROCEDURE RejectCount(VAR f: Fsm): CARDINAL;
BEGIN
  RETURN f.rejected
END RejectCount;

PROCEDURE ErrorCount(VAR f: Fsm): CARDINAL;
BEGIN
  RETURN f.errors
END ErrorCount;

(* ── Table helpers ───────────────────────────────────── *)

PROCEDURE SetTrans(VAR t: Transition; next: StateId;
                   action: ActionId; guard: GuardId);
BEGIN
  t.next := next;
  t.action := action;
  t.guard := guard
END SetTrans;

PROCEDURE ClearTable(trans: ADDRESS; n: CARDINAL);
VAR tp: TransPtr; i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < n DO
    tp := TransPtr(LONGCARD(trans)
          + LONGCARD(i * TSIZE(Transition)));
    tp^.next := NoState;
    tp^.action := NoAction;
    tp^.guard := NoGuard;
    INC(i)
  END
END ClearTable;

END Fsm.
