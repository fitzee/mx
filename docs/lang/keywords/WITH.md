# WITH

Record field access shorthand. Fields of the record variable are accessible
without qualification inside the WITH body.

```modula2
WITH recordVar DO
  statements  (* fields accessible directly *)
END;
```

## Example

```modula2
WITH point DO
  x := 10;
  y := 20;
  dist := Sqrt(FLOAT(x * x + y * y));
END;
```
