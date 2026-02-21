# MathLib0

Standard mathematical functions. Provides common floating-point
operations following the PIM4 library convention.

## Exported Procedures

```modula2
PROCEDURE sqrt(x: REAL): REAL;
PROCEDURE exp(x: REAL): REAL;
PROCEDURE ln(x: REAL): REAL;
PROCEDURE sin(x: REAL): REAL;
PROCEDURE cos(x: REAL): REAL;
PROCEDURE arctan(x: REAL): REAL;
PROCEDURE entier(x: REAL): INTEGER;
PROCEDURE real(n: INTEGER): REAL;
```

## Notes

- `entier` returns the largest integer not greater than `x` (floor).
- `real` converts an integer to its floating-point representation.
- `ln` is the natural logarithm (base e).
- Angles for `sin`, `cos`, and `arctan` are in radians.

## Example

```modula2
MODULE MathDemo;
FROM MathLib0 IMPORT sqrt, sin, cos, real;
FROM InOut IMPORT WriteString, WriteLn;
FROM RealInOut IMPORT WriteReal;
VAR hyp: REAL;
BEGIN
  hyp := sqrt(real(3) * real(3) + real(4) * real(4));
  WriteString("Hypotenuse: ");
  WriteReal(hyp, 10);
  WriteLn;
END MathDemo.
```
