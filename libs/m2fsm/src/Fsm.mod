IMPLEMENTATION MODULE Fsm;

FROM SYSTEM IMPORT ADDRESS, TSIZE;

(* ── Internal overlay types for array access ─────────── *)

TYPE
  TransArr  = ARRAY [0..16383] OF Transition;
  TransPtr  = POINTER TO TransArr;
  ActArr    = ARRAY [0..255] OF ActionProc;
  ActArrPtr = POINTER TO ActArr;
  GrdArr    = ARRAY [0..255] OF GuardProc;
  GrdArrPtr = POINTER TO GrdArr;
  HkArr     = ARRAY [0..255] OF HookProc;
  HkArrPtr  = POINTER TO HkArr;

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

PROCEDURE Step(VAR f: Fsm; ev: EventId; payload: ADDRESS;
               VAR status: StepStatus);
VAR
  tp: TransPtr;
  ap: ActArrPtr;
  gp: GrdArrPtr;
  hp: HkArrPtr;
  act: ActionProc;
  grd: GuardProc;
  hook: HookProc;
  t: Transition;
  fromState: StateId;
  idx: CARDINAL;
  allow, ok: BOOLEAN;
BEGIN
  fromState := f.state;

  (* Bounds check *)
  IF (f.state >= f.numStates) OR (ev >= f.numEvents) THEN
    status := Error;
    INC(f.errors);
    DoTrace(f, fromState, f.state, ev, NoAction, Error);
    RETURN
  END;

  (* Lookup transition *)
  idx := f.state * f.numEvents + ev;
  tp := f.trans;
  t := tp^[idx];

  IF t.next = NoState THEN
    status := NoTransition;
    INC(f.invalid);
    DoTrace(f, fromState, f.state, ev, NoAction, NoTransition);
    RETURN
  END;

  (* Guard check *)
  IF (t.guard # NoGuard) AND (f.guards # NIL) AND
     (t.guard < f.numGuards) THEN
    gp := f.guards;
    grd := gp^[t.guard];
    IF grd # NIL THEN
      allow := TRUE;
      grd(f.ctx, ev, payload, allow);
      IF NOT allow THEN
        status := GuardRejected;
        INC(f.rejected);
        DoTrace(f, fromState, f.state, ev, t.action, GuardRejected);
        RETURN
      END
    END
  END;

  (* Exit hook *)
  IF f.onExit # NIL THEN
    hp := f.onExit;
    IF f.state < f.numStates THEN
      hook := hp^[f.state];
      IF hook # NIL THEN
        hook(f.ctx, f.state, payload)
      END
    END
  END;

  (* Transition *)
  f.state := t.next;

  (* Enter hook *)
  IF f.onEnter # NIL THEN
    hp := f.onEnter;
    IF f.state < f.numStates THEN
      hook := hp^[f.state];
      IF hook # NIL THEN
        hook(f.ctx, f.state, payload)
      END
    END
  END;

  (* Action *)
  IF (t.action # NoAction) AND (f.acts # NIL) AND
     (t.action < f.numActs) THEN
    ap := f.acts;
    act := ap^[t.action];
    IF act # NIL THEN
      ok := TRUE;
      act(f.ctx, ev, payload, ok);
      IF NOT ok THEN
        status := Error;
        INC(f.errors);
        DoTrace(f, fromState, f.state, ev, t.action, Error);
        RETURN
      END
    END
  END;

  status := Ok;
  INC(f.steps);
  DoTrace(f, fromState, f.state, ev, t.action, Ok)
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
  tp := trans;
  i := 0;
  WHILE i < n DO
    tp^[i].next := NoState;
    tp^[i].action := NoAction;
    tp^[i].guard := NoGuard;
    INC(i)
  END
END ClearTable;

END Fsm.
