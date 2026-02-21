# PROCEDURE

Procedure or function declaration. Parameters can be passed by value or by
reference (VAR). Functions return a value via RETURN.

```modula2
PROCEDURE name(param1: T1; VAR param2: T2): ReturnType;
  (* local declarations *)
BEGIN
  (* statements *)
  RETURN expr;
END name;
```

## Example

```modula2
PROCEDURE Max(a, b: INTEGER): INTEGER;
BEGIN
  IF a > b THEN RETURN a ELSE RETURN b END;
END Max;
```
