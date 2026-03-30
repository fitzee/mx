# Storage

Heap memory allocation and deallocation. This module provides the `ALLOCATE` and `DEALLOCATE` procedures that are called implicitly by the built-in `NEW` and `DISPOSE` operations. You rarely call these directly -- import Storage to enable `NEW`/`DISPOSE`, and the compiler handles the rest.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

```modula2
PROCEDURE ALLOCATE(VAR p: ADDRESS; size: CARDINAL);
```
Allocate `size` bytes of heap memory and store the pointer in `p`. This is called automatically by `NEW(p)` -- the compiler inserts the call with the correct size based on the pointed-to type. You generally do not need to call this directly.

```modula2
PROCEDURE DEALLOCATE(VAR p: ADDRESS; size: CARDINAL);
```
Free the heap block at `p`. Called automatically by `DISPOSE(p)`. After deallocation, `p` is set to `NIL`.

## How NEW and DISPOSE work

When you write `NEW(p)` where `p` is a `POINTER TO SomeRecord`, the compiler generates a call to `ALLOCATE(p, SIZE(SomeRecord))`. Similarly, `DISPOSE(p)` generates `DEALLOCATE(p, SIZE(SomeRecord))`. For this to work, `ALLOCATE` and `DEALLOCATE` must be visible in the current scope -- which means you need to import them from Storage.

## Example

```modula2
MODULE HeapDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    val: INTEGER;
    next: NodePtr
  END;

VAR p, q: NodePtr;

BEGIN
  (* NEW calls ALLOCATE internally *)
  NEW(p);
  p^.val := 42;
  p^.next := NIL;

  NEW(q);
  q^.val := 99;
  q^.next := p;

  (* Walk the list *)
  WriteString("List: ");
  WriteInt(q^.val, 1);
  WriteString(" -> ");
  WriteInt(q^.next^.val, 1);
  WriteLn;

  (* DISPOSE calls DEALLOCATE internally *)
  DISPOSE(q);
  DISPOSE(p)
END HeapDemo.
```
