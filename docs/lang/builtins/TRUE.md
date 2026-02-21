# TRUE

```modula2
TRUE
```

The boolean constant representing the true value. `TRUE` is a pervasive constant of type `BOOLEAN`.

## Example

```modula2
VAR done: BOOLEAN;

BEGIN
  done := FALSE;
  WHILE NOT done DO
    (* ... *)
    done := TRUE;
  END;
  IF done = TRUE THEN (* redundant but valid *) END;
END
```

## Notes

- `ORD(TRUE) = 1`.
- `TRUE` is the result of relational expressions that hold (e.g., `3 > 2`).
- `MAX(BOOLEAN) = TRUE`.
- Boolean operators: `AND`, `OR`, `NOT` operate on `BOOLEAN` values.
- Comparing with `TRUE` explicitly (e.g., `IF x = TRUE`) is valid but idiomatic Modula-2 prefers `IF x`.
