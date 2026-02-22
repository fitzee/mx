# FsmTrace

Console trace adapter for the Fsm module. Provides a ready-made `TraceProc` that prints one line per `Step` to stdout via InOut.

## Why FsmTrace?

The Fsm module accepts an optional trace callback but does not include any output logic itself. FsmTrace fills that gap for development and debugging without requiring m2Log or any other dependency.

## Procedures

### ConsoleTrace

```modula2
PROCEDURE ConsoleTrace(traceCtx: ADDRESS;
                       fromState, toState: StateId;
                       ev: EventId; action: ActionId;
                       status: StepStatus);
```

Print a human-readable trace line to stdout. Compatible with the `Fsm.TraceProc` type signature.

Output format:

```
FSM: <from> -> <to> ev=<event> act=<action> <STATUS>
```

Where `<STATUS>` is one of `OK`, `NO_TRANS`, `REJECTED`, or `ERROR`.

## Usage

```modula2
FROM FsmTrace IMPORT ConsoleTrace;
...
Fsm.SetTrace(f, ConsoleTrace, NIL);
```
