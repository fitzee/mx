# SAFE

Safe module annotation. Declares that a module does not use unsafe operations. Requires `--m2plus`.

## Syntax

```modula2
SAFE MODULE ModuleName;
  (* module body *)
END ModuleName.
```

## Notes

- A SAFE module should not import from SYSTEM or perform unchecked type casts, address arithmetic, or other unsafe operations.
- The annotation documents intent and enables future compile-time enforcement of safety constraints.
- Currently parsed and accepted by the compiler but not enforced -- unsafe operations inside a SAFE module are not rejected.
- Use SAFE to communicate to readers and tools that the module is intended to be memory-safe.

## Example

```modula2
SAFE MODULE SafeStack;
FROM InOut IMPORT WriteString, WriteLn;

TYPE
  Stack = RECORD
    data: ARRAY [0..99] OF INTEGER;
    top: INTEGER;
  END;

PROCEDURE Push(VAR s: Stack; val: INTEGER);
BEGIN
  s.data[s.top] := val;
  INC(s.top);
END Push;

END SafeStack.
```
