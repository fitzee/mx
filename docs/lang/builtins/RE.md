# RE

```modula2
RE(z): REAL
```

Extract the real part of a `COMPLEX` value `z`.

`z` must be of type `COMPLEX`.

## Example

```modula2
VAR z: COMPLEX;
    r: REAL;

BEGIN
  z := CMPLX(3.0, 4.0);
  r := RE(z);   (* r = 3.0 *)
END
```

## Notes

- `RE` is an ISO 10514 extension and is not part of the original PIM4 standard.
- See also `IM` for extracting the imaginary part and `CMPLX` for constructing complex values.
