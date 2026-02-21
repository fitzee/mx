# BEGIN

Marks the start of the statement block in a module or procedure body.
Separates declarations from executable statements.

```modula2
PROCEDURE Foo;
  (* declarations *)
BEGIN
  (* statements *)
END Foo;
```

## Example

```modula2
MODULE Init;
VAR x: INTEGER;
BEGIN
  x := 42;
END Init.
```
