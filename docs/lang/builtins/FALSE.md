# FALSE

```modula2
FALSE
```

The boolean constant representing the false value. `FALSE` is a pervasive constant of type `BOOLEAN`.

## Example

```modula2
VAR found: BOOLEAN;
    i: CARDINAL;

BEGIN
  found := FALSE;
  FOR i := 0 TO HIGH(data) DO
    IF data[i] = target THEN
      found := TRUE;
    END;
  END;
END
```

## Notes

- `ORD(FALSE) = 0`.
- `FALSE` is the result of relational expressions that do not hold (e.g., `2 > 3`).
- `MIN(BOOLEAN) = FALSE`.
- `NOT TRUE = FALSE` and `NOT FALSE = TRUE`.
