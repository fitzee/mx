# Design Rationale

## Dense Table vs Sparse List

The core decision: how to store transitions.

**Dense table** (chosen): a flat array of `numStates * numEvents` entries. Lookup is `table[state * numEvents + event]` -- O(1), one multiply and one add. Empty slots use `NoState` sentinel. Memory cost is `numStates * numEvents * sizeof(Transition)` = 12 bytes per entry.

**Sparse list**: a sorted or hashed list of (state, event) pairs. Lower memory for FSMs with few transitions per state, but O(log n) or O(1)-amortized lookup with more complexity.

For protocol stacks (HTTP/2: ~10 states, ~15 events = 150 entries = 1.8 KB), the dense table is trivially small. Even 64 states x 64 events = 48 KB, which fits comfortably on the stack. The simplicity and determinism of dense lookup wins.

## Callback Model

All callbacks use procedure types with ADDRESS parameters for context and payload. This avoids forcing the user into a specific type hierarchy or generic framework.

- **ActionProc**: fires after state change, returns ok. Failure does NOT roll back the state change (rolling back would require calling onExit/onEnter again, creating confusing ordering).
- **GuardProc**: fires before state change, returns allow. Rejection is clean -- no side effects.
- **HookProc**: fires on entry/exit. No return value -- hooks are notifications.
- **TraceProc**: fires exactly once per Step, always, regardless of outcome.

## No Heap Allocation

The FSM stores ADDRESS pointers to caller-provided arrays. The caller controls memory layout entirely -- stack arrays, arena-allocated, or even statically initialized.

## Sentinel Strategy

- `NoState = 4294967295` (0xFFFFFFFF): marks empty transition table entries
- `NoAction = 0`: action index 0 is reserved. Real actions start at index 1.
- `NoGuard = 0`: same convention as actions.

This lets users declare arrays as `ARRAY [0..N] OF ActionProc` where slot 0 is NIL and slots 1..N are real callbacks. It wastes one slot but keeps indexing simple.

## Error Semantics

When an action returns `ok=FALSE`, the state has already changed. This is documented. The rationale: onExit and onEnter hooks have already fired. Rolling back would mean calling onExit on the new state and onEnter on the old state, which is confusing and non-deterministic. The caller can use the Error status to decide whether to reset the FSM.

## Instrumentation

Four counters (steps, invalid, rejected, errors) cover all Step outcomes. Combined with the trace callback, this provides full observability without any heap or I/O dependency in the core module.
