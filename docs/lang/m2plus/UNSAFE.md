# UNSAFE

Unsafe module annotation. Declares that a module explicitly uses
unsafe operations. Requires `--m2plus`.

## Syntax

```modula2
UNSAFE MODULE ModuleName;
  (* module body *)
END ModuleName.
```

## Notes

- An UNSAFE module may freely import from SYSTEM, perform address
  arithmetic, unchecked type casts, and other low-level operations.
- The annotation documents that the module intentionally opts into
  unsafe operations for performance or FFI reasons.
- Currently parsed and accepted by the compiler but not enforced --
  the UNSAFE annotation has no effect on compilation.
- Pair with SAFE modules to clearly separate safe and unsafe code
  in a project.

## Example

```modula2
UNSAFE MODULE RawMemory;
FROM SYSTEM IMPORT ADR, ADDRESS, TSIZE, BYTE;

PROCEDURE CopyBytes(src, dst: ADDRESS; n: CARDINAL);
VAR i: CARDINAL;
BEGIN
  FOR i := 0 TO n - 1 DO
    (* low-level byte copy *)
  END;
END CopyBytes;

END RawMemory.
```
