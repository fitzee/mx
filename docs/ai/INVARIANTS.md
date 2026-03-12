# mx House Style Invariants

## Purpose

This is the short, prescriptive version of the invariants document for
mx runtime, compiler, async, networking, and systems libraries.

These are house rules meant to remove ambiguity during implementation
and review.

------------------------------------------------------------------------

## 1. Ownership must be obvious

Every allocated object must have one of these ownership modes:

-   unique owner
-   shared owner via retain/release or refcount
-   borrowed, non‑owning reference
-   transferred ownership

If a reviewer cannot determine who frees an object quickly, the code is
incomplete.

------------------------------------------------------------------------

## 2. One type, one allocation discipline

Each concrete type should use a single primary allocation model:

Allowed patterns:

-   `NEW` / `DISPOSE`
-   pool allocation / pool free
-   arena allocation with arena-wide teardown

Do not mix these casually for the same record type.

------------------------------------------------------------------------

## 3. One type, one reclamation path

Each complex type must have one clear reclamation path.

Examples:

-   `Release` / `TryReclaim`
-   `FreeCont`
-   `ReleaseCancel`

Avoid scattered frees across unrelated procedures.

------------------------------------------------------------------------

## 4. Public handles must not outlive backing storage

If a public handle is a raw pointer, its lifetime must be protected by
ownership rules.

This requires one of:

-   counted reference ownership
-   explicit alias contract
-   public release procedure that invalidates the handle

------------------------------------------------------------------------

## 5. Release procedures must nil caller handles

Public release procedures should take `VAR` parameters and clear the
caller's handle.

Example:

    PROCEDURE FutureRelease(VAR f: Future);

Implementation pattern:

    sh := f;
    f := NIL;
    Release(sh)

------------------------------------------------------------------------

## 6. Pointer states

Pointer fields must always be in one of these states:

-   valid owning pointer
-   valid borrowed pointer
-   NIL

Never rely on "probably still valid".

------------------------------------------------------------------------

## 7. Validate cheap invariants early

At entry to public procedures validate:

-   NIL inputs
-   bounds
-   range values
-   already‑settled state

Reject invalid inputs early.

------------------------------------------------------------------------

## 8. Capture link pointers before enqueue

If nodes may be scheduled or freed after enqueue, capture `next` before
handoff.

    next := c^.next;
    SchedulerEnqueue(...);
    c := next;

Never read `c^.next` after enqueue.

------------------------------------------------------------------------

## 9. Head/tail invariants

For intrusive lists:

-   if `head = NIL` then `tail = NIL`
-   appending to empty sets both
-   detaching clears both
-   restoring remaining chain repairs both

------------------------------------------------------------------------

## 10. Refcounts count real owners

Refcounts must correspond to actual ownership edges.

Each increment must correspond to a real owner. Each decrement must
correspond to that owner going away.

------------------------------------------------------------------------

## 11. Every retain must match a release

Review code for:

-   success path
-   early failure
-   mid‑construction failure
-   repeated calls

Each retain must have a matching release.

------------------------------------------------------------------------

## 12. Queued work must keep dependencies alive

If future work dereferences an object, the object must remain alive
until that work executes.

Possible anchors:

-   refcount ownership
-   queue membership
-   stronger documented lifetime guarantees

------------------------------------------------------------------------

## 13. Scheduler model must be explicit

Each module must clearly state:

-   single‑threaded scheduler confinement
-   or cross‑thread access assumptions

------------------------------------------------------------------------

## 14. Callback dispatch rules must be explicit

Define:

-   dispatch order
-   inline vs scheduled dispatch
-   how many callbacks run per scheduler step
-   behavior when callbacks add new callbacks

------------------------------------------------------------------------

## 15. Best‑effort vs atomic construction

Async combinators must choose one:

-   atomic construction
-   best‑effort construction

If best‑effort, document what may remain alive after failure.

------------------------------------------------------------------------

## 16. Failure paths are part of the implementation

Every procedure that allocates, appends, or enqueues must be correct
when failure occurs mid‑execution.

------------------------------------------------------------------------

## 17. Defensive initialization around callbacks

Before invoking external callbacks, initialize output records.

    InitResult(outRes);
    fn(...);

------------------------------------------------------------------------

## 18. ADDRESS requires tagging

If a field of type `ADDRESS` may contain multiple record types, store a
discriminator/tag alongside it.

------------------------------------------------------------------------

## 19. Pool slots must be scrubbed

Before returning pooled records to free storage clear:

-   pointers
-   counters
-   links
-   function pointers
-   tags

------------------------------------------------------------------------

## 20. Do not bypass the ownership model

Once a module uses `Release` or `TryReclaim`, public API code must use
that model rather than raw frees.

------------------------------------------------------------------------

## Review checklist

Before merging:

### Ownership

-   Who owns each allocation?
-   What releases it?
-   Can handles outlive storage?

### Refcounts

-   What exactly does the refcount represent?
-   Does every retain match a release?

### Lists and queues

-   Are head/tail invariants preserved?
-   Is `next` captured before enqueue?

### Failure behavior

-   What happens if allocation fails?
-   What happens if enqueue fails?
-   Is construction atomic or best effort?

### API clarity

-   Are ownership rules documented?
-   Are borrowed pointers clearly labeled?
