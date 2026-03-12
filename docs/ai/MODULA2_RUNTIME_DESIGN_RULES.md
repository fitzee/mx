# MODULA2_RUNTIME_DESIGN_RULES.md

## Purpose

This document defines runtime design rules for mx-style Modula-2 systems code, especially code involving:

- schedulers
- continuations
- futures/promises
- cancellation
- intrusive queues/lists
- pooled runtime objects
- async combinators
- callback dispatch

It is narrower than `INVARIANTS.md`.
This is the rulebook for runtime mechanics.

---

## 1. State the execution model at the top of the module

Every runtime module must begin with a short invariants header stating:

- threading model
- scheduler model
- ownership model
- dispatch model
- reclamation model
- whether construction is atomic or best effort

If this is not written down, the module is not finished.

---

## 2. Pick one lifetime anchor per relationship

For each relationship in async/runtime code, decide what keeps an object alive:

- refcount ownership
- queue membership
- external handle ownership
- dispatcher-owned reference
- arena lifetime

Do not mix these accidentally.

Typical examples:

- output future lifetime -> refcount
- input future while continuations are queued on it -> queue membership
- cancellation token during scheduled callback drain -> dispatch-held ref
- public handle lifetime -> explicit external ref

Every dereference in a scheduled callback must have a visible lifetime anchor.

---

## 3. Queue membership may act as a lifetime anchor only if documented

This pattern is valid:

- continuations are stored on `inSh.contHead`
- `TryReclaim(inSh)` refuses reclamation while `contHead # NIL`
- therefore queued continuations keep `inSh` alive indirectly

But this must be called out explicitly.

If queue membership is the lifetime anchor, document it in module comments. Otherwise later refactors will break it.

---

## 4. Scheduled work must never outlive its storage

If a callback/task/continuation is queued, all objects it will dereference later must remain alive until it runs or is cancelled.

Valid ways to ensure that:

- retain/release around scheduled work
- queue membership on the owning structure
- explicit dispatcher-owned reference
- stronger enclosing runtime lifetime

Invalid pattern:

- schedule callback with raw pointer
- free/recycle the pointed object before the scheduler runs it

That is the classic runtime bug.

---

## 5. Capture `next` before enqueue boundaries

In intrusive queue/list traversal, always save `next` before enqueueing or calling code that may free/mutate the node.

Required pattern:

```modula2
next := c^.next;
st := SchedulerEnqueue(s, proc, c);
c := next;
```

Never do this:

```modula2
st := SchedulerEnqueue(s, proc, c);
c := c^.next;
```

Assume the scheduler may run immediately unless the contract proves otherwise.

---

## 6. Dispatch state must be explicit

For callback drainers, continuation executors, and cancellation dispatchers, encode dispatch state in fields.

Typical fields:

- `dispatching: BOOLEAN`
- `cbNext: INTEGER`
- `cbCount: INTEGER`
- `settled: BOOLEAN`
- `failed: BOOLEAN`

Do not rely on comments or informal sequencing to represent runtime state.

If the state matters, it needs a field.

---

## 7. Callback append-during-dispatch must be defined

If callbacks can be appended while dispatch is already in progress, define the behavior explicitly.

Preferred model:

- callbacks append at the end
- `cbNext` advances monotonically
- active dispatcher keeps draining until `cbNext >= cbCount`
- no reset of `cbNext` while already dispatching

This avoids skipped callbacks, duplicate dispatch, and ordering anomalies.

---

## 8. Use dispatch-held refs for scheduled callback drains

If a scheduler callback may run later, and the owning handle can be destroyed before it runs, the dispatcher must hold its own reference.

Canonical example:

- `Cancel(ct)` wants to schedule `ExecCancelCB`
- caller may immediately `CancelTokenDestroy(ct)`
- therefore dispatch must retain the token before first enqueue
- final drain step releases the dispatch ref

Without this, queued scheduler work can outlive pooled storage.

---

## 9. Reclamation must not race logical liveness

Do not reclaim state just because there are no external handles if internal runtime work still depends on it.

Examples of internal liveness:

- queued continuations
- active dispatchers
- pending scheduled tasks
- internal refcount-held combinator state

Reclamation conditions must account for all ways state is still live.

---

## 10. Best-effort vs atomic construction must be chosen, not drifted into

For async combinators like `All` and `Race`, define whether creation is:

- atomic, or
- best effort

### Atomic
If anything fails, no live work remains.

### Best effort
If construction fails partway, some already-attached work may continue running.

If best effort is chosen, document exactly:
- what may remain live
- whether background work may still settle internal state
- what handle, if any, the caller still receives

Ambiguity here causes nasty bugs and false assumptions.

---

## 11. Output-handle ownership must be separate from continuation ownership

For future/promise style systems:

- external handle owns one reference
- each continuation that will later settle/use the output owns one additional reference
- continuation releases its reference after execution
- caller releases external handle when done

Do not merge these implicitly.

The owner graph should be obvious.

---

## 12. Alias-pair handles must be documented as a contract

If two API handles alias the same backing state, define whether they represent:

- one external ownership unit, or
- two independent ownership units

Example:
- `Promise` and `Future` both alias the same `SharedRec`

If that alias pair represents only one external reference, the API must say clearly:

- release exactly one of them, not both

This is not intuitive and must be documented hard.

---

## 13. Public release procedures are mandatory for raw-pointer handles

If public handles are raw pointers into refcounted or pooled runtime objects, expose release procedures.

Pattern:

```modula2
PROCEDURE PromiseRelease(VAR p: Promise);
PROCEDURE FutureRelease(VAR f: Future);
```

Release procedures should:
- copy the pointer locally
- nil the caller handle
- release ownership

This reduces accidental double-release and stale use.

---

## 14. Settled state and delivery state are separate concerns

A future/promise may be:

- logically settled
- not fully delivered to all continuations yet

Do not conflate these.

Example:
- `SettleWith` may set result/fate
- draining continuation delivery may still partially fail due to enqueue exhaustion

This means:
- settlement state and delivery state are distinct
- if delivery can fail partially, document it

---

## 15. Status-returning runtime operations need a failure policy

For runtime calls like:
- enqueue
- allocation
- append/dispatch setup

the module must define whether failure means:
- full rollback
- partial live state remains
- work is stranded
- failure is fatal/system-level

Never silently ignore status returns unless the consequence is documented.

---

## 16. Pooled runtime nodes must be scrubbed aggressively

When returning pooled nodes to free storage, clear:

- pointers
- link fields
- counters
- function pointers
- discriminators
- ownership-related fields

Scheduler/async code is especially sensitive to stale pool state because reuse happens quickly.

---

## 17. Generic payloads need discriminators

If runtime records store generic payloads via `ADDRESS`, add a nearby discriminator or kind field.

Examples:
- `combKind` paired with `combSt`
- callback record kind paired with union-like fields

Do not guess concrete type at free/reclaim time.

---

## 18. Combinator result storage lifetime must be explicit

If a combinator returns a pointer into internal storage, document:

- what it points to
- who owns that storage
- how long it remains valid
- when caller must copy it

Example:
- `All` returning `ADR(results)` into output value

That is acceptable only with explicit lifetime rules.

---

## 19. Runtime code must be reviewable as state machines

For each runtime abstraction, the reviewer should be able to sketch the state machine.

Examples:

### Promise/Future
- `Pending -> Fulfilled`
- `Pending -> Rejected`

### Cancellation
- `cancelled = FALSE -> TRUE`

### Callback drain
- `dispatching = FALSE -> TRUE -> FALSE`

### Race
- `settled = FALSE -> TRUE`

If you cannot sketch the machine, the code is underspecified.

---

## 20. Callback wrappers must own what they capture

If a wrapper record is heap-allocated to adapt a callback:

- wrapper owns its own captured references
- wrapper frees itself exactly once
- any internal refs captured by wrapper are released exactly once

Typical example:
- `MapCancellable`
- wrapper retains cancel token
- wrapper callback releases token and disposes itself

That pattern is good. Leaking wrapper state is not.

---

## 21. Scheduler fairness vs completion guarantees must be explicit

If callback dispatch chooses “one callback per pump step” for fairness, document the tradeoff.

You must say:
- whether this improves fairness
- whether callbacks can remain pending across ticks
- what happens if re-enqueue fails mid-drain

Fairness decisions are runtime semantics, not implementation trivia.

---

## 22. Runtime docs must describe what happens after failure

Low-level async/runtime modules should document the post-failure world.

Examples:
- after partial `All` construction failure, some attached continuations may still run
- after scheduler enqueue failure during dispatch, remaining callbacks may be dropped or stranded
- after releasing the last external handle, pending abandoned state may be reclaimed

This is the difference between a usable runtime and a trap.

---

## 23. Do not hide lossy semantics

If callback registration can fail silently due to capacity limits, document it.

If the API returns no status, the docs must say:
- registration is bounded
- excess callbacks are dropped
- enqueue exhaustion may prevent callback delivery

Lossy behavior is acceptable in some runtimes. Hidden lossy behavior is not.

---

## 24. Runtime helper procedures should encode policy, not just mechanics

Helpers like:
- `SettleWith`
- `TryReclaim`
- `DrainConts`
- `ReleaseCancel`

should not be “bag of code” helpers.

They should encode actual runtime policy:
- when reclaim is legal
- how delivery is attempted
- what failure means
- what state transitions occur

If the policy is spread everywhere, the runtime becomes unreviewable.

---

## 25. Module-level review questions

Before merging runtime code, answer these questions directly.

### Lifetime
- What keeps each object alive?
- What releases it?
- Can queued work outlive storage?

### Scheduling
- Can any callback run inline unexpectedly?
- Is iteration safe if scheduled work runs immediately?
- Is dispatch state explicit?

### Failure
- What happens if enqueue fails here?
- What remains live after partial failure?
- Is this atomic or best effort?

### Handles
- Are public raw-pointer handles protected by ownership rules?
- Are alias semantics explicit?
- Are release procedures documented?

### Pools
- Are freed slots scrubbed?
- Could stale pointers refer to recycled pool slots?

---

## Final rule

A runtime module is done when a hostile reviewer can trace:

- who owns every runtime object
- what keeps scheduled work alive
- when state can be reclaimed
- what happens when allocation or enqueue fails
- which semantics are best effort

without guessing.

If they need to infer the model from scattered code, the runtime is not tight enough yet.
