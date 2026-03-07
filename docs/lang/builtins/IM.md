# IM

```modula2
IM(z): REAL
```

Extract the imaginary part of a `COMPLEX` value `z`.

`z` must be of type `COMPLEX`.

## Example

```modula2
VAR z: COMPLEX;
    r: REAL;

BEGIN
  z := CMPLX(3.0, 4.0);
  r := IM(z);   (* r = 4.0 *)
END
```

## Notes

- `IM` is an ISO 10514 extension and is not part of the original PIM4 standard.
- See also `RE` for extracting the real part and `CMPLX` for constructing complex values.
