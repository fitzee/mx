# Fsm

## Why
Provides a table-driven finite state machine with O(1) transition lookup, per-transition actions and guards, state entry/exit hooks, and built-in instrumentation -- all without heap allocation.

## Types

- **StateId** (CARDINAL) -- Numeric identifier for a state.
- **EventId** (CARDINAL) -- Numeric identifier for an event.
- **ActionId** (CARDINAL) -- Index into the actions array. 0 = no action.
- **GuardId** (CARDINAL) -- Index into the guards array. 0 = no guard.
- **StepStatus** -- Result of processing one event: `Ok`, `NoTransition`, `GuardRejected`, `Error`.
- **Transition** -- Record with fields `next: StateId`, `action: ActionId`, `guard: GuardId`.
- **Fsm** -- The FSM instance record holding state, transition table pointer, callbacks, and counters.
- **ActionProc** -- `PROCEDURE(ctx: ADDRESS, ev: EventId, payload: ADDRESS, VAR ok: BOOLEAN)` -- Called when a transition fires. Set ok to FALSE to signal error.
- **GuardProc** -- `PROCEDURE(ctx: ADDRESS, ev: EventId, payload: ADDRESS, VAR allow: BOOLEAN)` -- Called before transition. Set allow to FALSE to reject.
- **HookProc** -- `PROCEDURE(ctx: ADDRESS, state: StateId, payload: ADDRESS)` -- Called on state entry or exit.
- **TraceProc** -- `PROCEDURE(traceCtx: ADDRESS, fromState, toState: StateId, ev: EventId, action: ActionId, status: StepStatus)` -- Called once per Step for debugging.

## Constants

- `NoState = 4294967295` -- Sentinel meaning "no transition defined" in a table entry.
- `NoAction = 0` -- Reserved action index (no-op).
- `NoGuard = 0` -- Reserved guard index (no-op / always allow).

## Procedures

### Lifecycle

- `PROCEDURE Init(VAR f: Fsm; start: StateId; ctx: ADDRESS; numStates, numEvents: CARDINAL; trans: ADDRESS)`
  Initialise an FSM with a dense transition table. `trans` must point to an array of `numStates * numEvents` Transition records.

- `PROCEDURE Reset(VAR f: Fsm)`
  Reset state to start and clear all counters.

### Configuration

- `PROCEDURE SetActions(VAR f: Fsm; acts: ADDRESS; numActs: CARDINAL)`
  Set the action callback array. Index 0 is reserved; real actions start at index 1.

- `PROCEDURE SetGuards(VAR f: Fsm; guards: ADDRESS; numGuards: CARDINAL)`
  Set the guard callback array. Index 0 is reserved; real guards start at index 1.

- `PROCEDURE SetHooks(VAR f: Fsm; enterHooks, exitHooks: ADDRESS)`
  Set state entry/exit hook arrays (numStates HookProc values each, indexed by StateId). Pass NIL to disable.

- `PROCEDURE SetTrace(VAR f: Fsm; proc: TraceProc; traceCtx: ADDRESS)`
  Set a trace callback called once per Step. Pass NIL to disable tracing.

### Core

- `PROCEDURE Step(VAR f: Fsm; ev: EventId; payload: ADDRESS; VAR status: StepStatus)`
  Process one event. Performs bounds check, transition lookup, guard evaluation, exit/enter hooks, action execution, and tracing -- in that order. Updates instrumentation counters.

### Queries

- `PROCEDURE CurrentState(VAR f: Fsm): StateId` -- Current state.
- `PROCEDURE StepCount(VAR f: Fsm): CARDINAL` -- Number of successful transitions.
- `PROCEDURE InvalidCount(VAR f: Fsm): CARDINAL` -- Events with no matching transition.
- `PROCEDURE RejectCount(VAR f: Fsm): CARDINAL` -- Transitions rejected by guards.
- `PROCEDURE ErrorCount(VAR f: Fsm): CARDINAL` -- Action errors.

### Table Helpers

- `PROCEDURE SetTrans(VAR t: Transition; next: StateId; action: ActionId; guard: GuardId)`
  Fill a Transition record.

- `PROCEDURE ClearTable(trans: ADDRESS; n: CARDINAL)`
  Fill n Transition entries with NoState/NoAction/NoGuard.

## FsmTrace Module

The companion `FsmTrace` module provides a ready-made console trace adapter:

- `PROCEDURE ConsoleTrace(traceCtx: ADDRESS; fromState, toState: StateId; ev: EventId; action: ActionId; status: StepStatus)`
  Prints human-readable trace lines to stdout. Output format: `FSM: 0 -> 1 ev=0 act=1 OK`.

  Plug it in with: `Fsm.SetTrace(f, FsmTrace.ConsoleTrace, NIL)`.

## Example

```modula2
MODULE FsmExample;

FROM SYSTEM IMPORT ADR, ADDRESS;
FROM Fsm IMPORT Fsm, Transition, StepStatus, ActionProc,
                Init, Reset, SetActions, SetTrans, ClearTable, Step,
                NoGuard, CurrentState;
FROM FsmTrace IMPORT ConsoleTrace;

CONST
  NumStates = 3;
  NumEvents = 2;

VAR
  f: Fsm;
  table: ARRAY [0..5] OF Transition;  (* 3 states * 2 events *)
  acts: ARRAY [0..1] OF ActionProc;
  status: StepStatus;

  PROCEDURE OnTransit(ctx: ADDRESS; ev: CARDINAL;
                       payload: ADDRESS; VAR ok: BOOLEAN);
  BEGIN
    ok := TRUE;
  END OnTransit;

BEGIN
  (* Clear the table, then define transitions *)
  ClearTable(ADR(table), NumStates * NumEvents);

  (* State 0 + Event 0 => State 1, action 1 *)
  SetTrans(table[0*NumEvents + 0], 1, 1, NoGuard);

  (* State 1 + Event 1 => State 2, action 1 *)
  SetTrans(table[1*NumEvents + 1], 2, 1, NoGuard);

  (* State 2 + Event 0 => State 0, action 1 (cycle back) *)
  SetTrans(table[2*NumEvents + 0], 0, 1, NoGuard);

  (* Set up actions *)
  acts[0] := NIL;         (* index 0 = NoAction *)
  acts[1] := OnTransit;

  (* Initialise and configure *)
  Init(f, 0, NIL, NumStates, NumEvents, ADR(table));
  SetActions(f, ADR(acts), 2);
  Fsm.SetTrace(f, ConsoleTrace, NIL);

  (* Drive the FSM *)
  Step(f, 0, NIL, status);  (* 0 -> 1 *)
  Step(f, 1, NIL, status);  (* 1 -> 2 *)
  Step(f, 0, NIL, status);  (* 2 -> 0 *)
END FsmExample.
```
