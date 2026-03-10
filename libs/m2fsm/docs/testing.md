# Testing

## Test Structure

Tests are in `tests/fsm_tests.mod`. The module uses the standard Check() pattern with passed/failed/total counters.

## Test Groups

### 1. Basic Transitions (TestBasic)
3-state, 3-event FSM with two actions. Verifies the happy path: state changes correctly, actions fire, step counter increments.

### 2. NoTransition (TestNoTransition)
Steps with events that have no matching table entry. Verifies state is unchanged, `InvalidCount` increments, and status is `NoTransition`.

### 3. Guard Rejection (TestGuardReject)
A guard callback conditionally allows or rejects a transition. First pass allows (verifies Ok), then Reset + reject (verifies GuardRejected, state unchanged, `RejectCount` increments).

### 4. Hook Ordering (TestHooksOrder)
Uses `ctx = ADR(f)` so hooks can read `f.state` at call time. Proves:
- onExit sees the **old** state (called before state change)
- onEnter sees the **new** state (called after state change)
- Exit fires before Enter (verified via sequence log)

### 5. Action Failure (TestActionFail)
Action returns `ok=FALSE`. Verifies:
- Status is `Error`
- `ErrorCount` increments
- State **remains changed** (documented design choice)
- `StepCount` does **not** increment

### 6. Trace Correctness (TestTrace)
Trace callback captures (from, to, event, action, status) for both Ok and NoTransition outcomes. Verifies exact values and that trace is called exactly once per Step.

### 7. Reset (TestReset)
Verifies Reset restores start state and zeroes all counters.

### 8. Bounds Check (TestBounds)
Event ID >= numEvents. Verifies Error status and `ErrorCount` increment.

### 9. ClearTable (TestClearTable)
Verifies ClearTable fills entries with NoState/NoAction/NoGuard.

### 10. SetTrans (TestSetTrans)
Verifies SetTrans fills a Transition record's fields correctly.

## Adding New Tests

1. Write a new `PROCEDURE TestXxx;` with local FSM setup
2. Use `Check("xxx: description", condition)` for each assertion
3. Add the call to the module body (BEGIN section)
4. Recompile and run

## Running

```bash
cd libs/m2fsm
../../target/release/mx tests/fsm_tests.mod -I src -o fsm_tests
./fsm_tests
```
