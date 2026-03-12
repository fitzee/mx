# AI Code Review for Modula‑2

## Purpose

This prompt is designed for auditing Modula‑2 systems code such as:

-   runtime libraries
-   schedulers
-   async frameworks
-   networking code
-   compiler infrastructure

This is **not a style review**.\
This is a **correctness and lifetime audit**.

------------------------------------------------------------------------

## Reviewer Instructions

Review the code assuming:

-   raw pointers are used
-   intrusive lists may exist
-   object pools may be used
-   scheduler callbacks may run asynchronously
-   explicit ownership is required

Prioritize:

1.  ownership and lifetime
2.  pointer safety
3.  queue/list invariants
4.  allocation/free symmetry
5.  retain/release correctness
6.  failure‑path cleanup
7.  scheduler and callback hazards
8.  public API ownership clarity

------------------------------------------------------------------------

## Required Output Structure

### 1. Overall assessment

Classify the code as:

-   structurally sound
-   plausible but unsafe
-   mostly correct with sharp edges
-   production‑ready with documented limitations
-   fundamentally broken

------------------------------------------------------------------------

### 2. Critical correctness bugs

List issues causing:

-   use‑after‑free
-   double free
-   leaks breaking the lifetime model
-   pool slot reuse hazards
-   corrupted lists
-   invalid ownership transfer

For each bug explain:

-   the failure mode
-   the relevant code pattern
-   how to fix it

------------------------------------------------------------------------

### 3. Ownership and lifetime review

Determine:

-   who owns each object
-   whether ownership is unique or shared
-   how the object is released
-   whether handles can outlive storage

Flag ambiguous ownership.

------------------------------------------------------------------------

### 4. Pointer and pool safety

Check:

-   NIL checks
-   dereference safety
-   pool reuse hazards
-   stale pointer risks
-   field clearing on free

------------------------------------------------------------------------

### 5. Queue and list invariants

Verify:

-   head/tail consistency
-   append correctness
-   detach correctness
-   partial failure restoration
-   capturing `next` before enqueue

------------------------------------------------------------------------

### 6. Failure path audit

Evaluate behavior when:

-   allocation fails
-   enqueue fails
-   construction fails mid‑sequence

Determine whether the design is atomic or best‑effort.

------------------------------------------------------------------------

### 7. Scheduler and callback review

Inspect:

-   inline vs scheduled execution assumptions
-   reentrancy safety
-   callback mutation during dispatch
-   dependency lifetime during scheduling

------------------------------------------------------------------------

### 8. API sharp edges

Identify dangerous API semantics such as:

-   aliasing handles
-   borrowed pointer results
-   required manual release ordering
-   silent callback dropping

------------------------------------------------------------------------

### 9. Concrete fixes

Provide prioritized fixes:

-   fix immediately
-   recommended improvement
-   acceptable limitation

------------------------------------------------------------------------

### 10. Final verdict

Provide a direct final judgment in one paragraph.

------------------------------------------------------------------------

## Special Modula‑2 hazards to inspect

Watch for:

-   `NEW` without matching `DISPOSE`
-   pools without slot scrubbing
-   `ADR()` escaping internal storage
-   `ADDRESS` without discriminator tags
-   intrusive node free while still linked
-   aliasing through `VAR` parameters
-   ignoring status returns
-   enqueue‑before‑capture traversal bugs
