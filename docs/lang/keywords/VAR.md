# VAR

Variable declaration. Declares mutable storage. Can appear at module level or
inside a procedure. Also used in parameter lists for pass-by-reference.

```modula2
VAR
  name1, name2: Type;
  name3: AnotherType;
```

## Example

```modula2
VAR
  count: INTEGER;
  name: ARRAY [0..31] OF CHAR;
  done: BOOLEAN;
```
