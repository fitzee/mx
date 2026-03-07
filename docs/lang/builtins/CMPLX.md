# CMPLX

```modula2
CMPLX(re, im): COMPLEX
```

Construct a `COMPLEX` value from a real part `re` and an imaginary part `im`.

`re` and `im` must be of type `REAL` or `LONGREAL`.

## Example

```modula2
VAR z: COMPLEX;

BEGIN
  z := CMPLX(1.0, 2.0);   (* z = 1.0 + 2.0i *)
  z := CMPLX(0.0, 0.0);   (* z = 0.0 *)
END
```

## Notes

- `CMPLX` is an ISO 10514 extension and is not part of the original PIM4 standard.
- See also `RE` and `IM` for extracting the real and imaginary parts of a `COMPLEX` value.
- Complex arithmetic support requires the `COMPLEX` type to be available in the implementation.
