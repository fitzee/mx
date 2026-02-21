# RETURN

Return from a procedure. In functions, an expression is required. In proper
procedures (no return type), RETURN takes no argument.

```modula2
RETURN;        (* proper procedure *)
RETURN expr;   (* function *)
```

## Example

```modula2
PROCEDURE Abs(x: INTEGER): INTEGER;
BEGIN
  IF x < 0 THEN RETURN -x ELSE RETURN x END;
END Abs;
```
