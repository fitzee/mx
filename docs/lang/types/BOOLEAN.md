# BOOLEAN

Logical type with exactly two values.

## Properties

- **Values**: `TRUE`, `FALSE`
- **Ordinal**: `ORD(FALSE) = 0`, `ORD(TRUE) = 1`
- **Operations**: `AND`, `OR`, `NOT`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `ORD`, `VAL`, `MIN`, `MAX`

## Syntax

```modula2
VAR
  done, found: BOOLEAN;

done := FALSE;
found := (x > 0) AND (x < 100);

IF NOT done OR found THEN
  done := TRUE
END;

WHILE NOT done DO
  (* ... *)
END;
```

## Notes

- `AND` and `OR` use short-circuit evaluation: the right operand is not evaluated if the left operand determines the result.
- BOOLEAN is an ordinal type, so `FALSE < TRUE`.
- `INC(b)` and `DEC(b)` are not meaningful on BOOLEAN and should be avoided.
- Relational operators on any comparable type produce BOOLEAN results.
- BOOLEAN values can be used directly as conditions; no comparison to TRUE is needed.
