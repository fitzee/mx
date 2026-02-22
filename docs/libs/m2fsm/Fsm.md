# Fsm

Table-driven finite state machine with O(1) transition lookup, optional guards, per-transition actions, state entry/exit hooks, and trace instrumentation. No heap allocation in the core runtime -- all storage is caller-provided.

## Why Fsm?

Protocol stacks (HTTP/2, TLS, RPC), stream backpressure logic, and parsers all follow the same pattern: a set of states, a set of events, and a table of transitions between them. Writing these as ad-hoc `IF`/`CASE` chains is fragile and hard to test. Fsm factors the state machine into a dense lookup table where adding a transition is one `SetTrans` call, and the runtime `Step` is O(1) with deterministic callback ordering.

The library does not allocate heap memory. Transition tables, action arrays, guard arrays, and hook arrays are all stack-allocated by the caller and passed to the FSM by address.

## Types

### StateId, EventId, ActionId, GuardId

```modula2
TYPE
  StateId  = CARDINAL;
  EventId  = CARDINAL;
  ActionId = CARDINAL;
  GuardId  = CARDINAL;
```

Numeric identifiers. Use named constants (e.g., `CONST StIdle = 0;`) for readability.

### StepStatus

```modula2
TYPE StepStatus = (Ok, NoTransition, GuardRejected, Error);
```

| Value | Meaning |
|-------|---------|
| `Ok` | Transition succeeded |
| `NoTransition` | No matching entry in the table |
| `GuardRejected` | Guard callback rejected the transition |
| `Error` | Out-of-range state/event, or action returned `ok=FALSE` |

### Transition

```modula2
TYPE Transition = RECORD
  next:   StateId;
  action: ActionId;
  guard:  GuardId;
END;
```

One entry in the dense transition table. `next=NoState` means no transition defined for this (state, event) pair.

### Callback Types

```modula2
TYPE
  ActionProc = PROCEDURE(ADDRESS, EventId, ADDRESS, VAR BOOLEAN);
  GuardProc  = PROCEDURE(ADDRESS, EventId, ADDRESS, VAR BOOLEAN);
  HookProc   = PROCEDURE(ADDRESS, StateId, ADDRESS);
  TraceProc  = PROCEDURE(ADDRESS, StateId, StateId, EventId,
                          ActionId, StepStatus);
```

| Type | Parameters | Purpose |
|------|------------|---------|
| `ActionProc` | ctx, event, payload, VAR ok | Transition action; set ok=FALSE to signal error |
| `GuardProc` | ctx, event, payload, VAR allow | Pre-transition guard; set allow=FALSE to reject |
| `HookProc` | ctx, state, payload | State entry/exit notification |
| `TraceProc` | traceCtx, fromState, toState, event, action, status | Debugging/logging |

### Fsm

```modula2
TYPE Fsm = RECORD
  state, start: StateId;
  ctx: ADDRESS;
  trans: ADDRESS;
  numStates, numEvents: CARDINAL;
  acts: ADDRESS;
  numActs: CARDINAL;
  guards: ADDRESS;
  numGuards: CARDINAL;
  onEnter, onExit: ADDRESS;
  trace: TraceProc;
  traceCtx: ADDRESS;
  steps, invalid, rejected, errors: CARDINAL;
END;
```

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `NoState` | 4294967295 | Sentinel: no transition defined |
| `NoAction` | 0 | Action slot 0 is reserved (no-op) |
| `NoGuard` | 0 | Guard slot 0 is reserved (no-op) |

## Procedures

### Init

```modula2
PROCEDURE Init(VAR f: Fsm; start: StateId; ctx: ADDRESS;
               numStates, numEvents: CARDINAL; trans: ADDRESS);
```

Initialise an FSM. `trans` must point to an array of `numStates * numEvents` Transition records (caller passes `ADR(myTable)`). Sets state to `start`, zeroes all counters, clears actions/guards/hooks/trace.

### Reset

```modula2
PROCEDURE Reset(VAR f: Fsm);
```

Reset state to start. Clear all counters. Configuration (actions, guards, hooks, trace) is preserved.

### SetActions

```modula2
PROCEDURE SetActions(VAR f: Fsm; acts: ADDRESS; numActs: CARDINAL);
```

Set the action callback array. `acts` points to an array of `numActs` ActionProc values. Index 0 is reserved (`NoAction`); real actions start at index 1. NIL entries are treated as no-op.

### SetGuards

```modula2
PROCEDURE SetGuards(VAR f: Fsm; guards: ADDRESS; numGuards: CARDINAL);
```

Set the guard callback array. Same conventions as actions. Index 0 is reserved (`NoGuard`).

### SetHooks

```modula2
PROCEDURE SetHooks(VAR f: Fsm; enterHooks, exitHooks: ADDRESS);
```

Set state entry/exit hook arrays. Each must point to an array of `numStates` HookProc values, indexed by StateId. NIL entries are no-op. Pass NIL for either pointer to disable that hook class.

### SetTrace

```modula2
PROCEDURE SetTrace(VAR f: Fsm; proc: TraceProc; traceCtx: ADDRESS);
```

Set the trace callback. Called exactly once per `Step` with the full transition context. Pass NIL to disable.

### Step

```modula2
PROCEDURE Step(VAR f: Fsm; ev: EventId; payload: ADDRESS;
               VAR status: StepStatus);
```

Process one event. Execution order:

1. Bounds-check state/event. Out of range => `Error`.
2. Lookup transition. `NoState` => `NoTransition`.
3. If guard exists and rejects => `GuardRejected`.
4. Call onExit hook for current state.
5. Change state.
6. Call onEnter hook for new state.
7. If action exists, call it. `ok=FALSE` => `Error` (state remains changed).
8. Otherwise => `Ok`.

Trace is called exactly once at the end.

### CurrentState / StepCount / InvalidCount / RejectCount / ErrorCount

```modula2
PROCEDURE CurrentState(VAR f: Fsm): StateId;
PROCEDURE StepCount(VAR f: Fsm): CARDINAL;
PROCEDURE InvalidCount(VAR f: Fsm): CARDINAL;
PROCEDURE RejectCount(VAR f: Fsm): CARDINAL;
PROCEDURE ErrorCount(VAR f: Fsm): CARDINAL;
```

Query counters. `StepCount` counts successful transitions (`Ok`). The other counters correspond to `NoTransition`, `GuardRejected`, and `Error` statuses respectively.

### SetTrans / ClearTable

```modula2
PROCEDURE SetTrans(VAR t: Transition; next: StateId;
                   action: ActionId; guard: GuardId);
PROCEDURE ClearTable(trans: ADDRESS; n: CARDINAL);
```

Table helpers. `ClearTable` fills `n` entries with `NoState`/`NoAction`/`NoGuard`. `SetTrans` fills one entry.

## Example

```modula2
CONST NumStates = 3; NumEvents = 3;
VAR f: Fsm;
    table: ARRAY [0..8] OF Transition;
    acts: ARRAY [0..1] OF ActionProc;
    status: StepStatus;

Fsm.ClearTable(ADR(table), 9);
Fsm.SetTrans(table[0*3+0], 1, 1, NoGuard);  (* S0+E0 -> S1, action 1 *)
acts[0] := NIL;
acts[1] := MyAction;
Fsm.Init(f, 0, NIL, 3, 3, ADR(table));
Fsm.SetActions(f, ADR(acts), 2);
Fsm.Step(f, 0, NIL, status);  (* status = Ok, state = 1 *)
```
