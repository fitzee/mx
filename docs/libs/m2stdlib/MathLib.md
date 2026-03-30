# MathLib

Standard mathematical functions operating on REAL (single-precision float) values. Provides trigonometric, exponential, and random number functions.

`MathLib` and `MathLib0` are identical -- they export the same procedures with the same signatures. Both names exist because different PIM4 compilers historically used one or the other. You can import from either; `FROM MathLib IMPORT sqrt` and `FROM MathLib0 IMPORT sqrt` are interchangeable.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

### Trigonometric

```modula2
PROCEDURE sin(x: REAL): REAL;
```
Sine of `x` (radians).

```modula2
PROCEDURE cos(x: REAL): REAL;
```
Cosine of `x` (radians).

```modula2
PROCEDURE arctan(x: REAL): REAL;
```
Arctangent of `x`, returning a value in radians. To compute atan2(y,x), use `arctan(y/x)` with appropriate quadrant handling.

### Exponential and Logarithmic

```modula2
PROCEDURE exp(x: REAL): REAL;
```
Natural exponential: e raised to the power `x`.

```modula2
PROCEDURE ln(x: REAL): REAL;
```
Natural logarithm of `x`. `x` must be positive.

### Root

```modula2
PROCEDURE sqrt(x: REAL): REAL;
```
Square root of `x`. `x` must be non-negative.

### Rounding

```modula2
PROCEDURE entier(x: REAL): INTEGER;
```
Floor function: returns the largest integer not greater than `x`. This is useful for converting REAL to INTEGER since Modula-2 has no implicit conversions. Note that `TRUNC` (a builtin) truncates toward zero, while `entier` rounds toward negative infinity.

```modula2
entier(3.7)    (* = 3 *)
entier(-3.7)   (* = -4, not -3 *)
TRUNC(-3.7)    (* = -3, truncates toward zero *)
```

### Random Numbers

```modula2
PROCEDURE Random(): REAL;
```
Return a pseudo-random REAL in the range [0.0, 1.0). Uses the C `rand()` function internally.

```modula2
PROCEDURE Randomize(seed: CARDINAL);
```
Seed the random number generator. Call this once at program start if you want different sequences on each run. Pass a value like the current time or process ID.

## Example

```modula2
MODULE MathDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM MathLib IMPORT sqrt, sin, cos, entier, Random, Randomize;

BEGIN
  WriteString("sqrt(144) = ");
  WriteInt(entier(sqrt(144.0)), 1); WriteLn;

  WriteString("sin(0) = ");
  WriteInt(entier(sin(0.0) * 1000.0), 1);
  WriteString(" (x1000)"); WriteLn;

  WriteString("cos(0) = ");
  WriteInt(entier(cos(0.0) * 1000.0), 1);
  WriteString(" (x1000)"); WriteLn;

  Randomize(42);
  WriteString("Random: ");
  WriteInt(entier(Random() * 100.0), 1);
  WriteString("%"); WriteLn
END MathDemo.
```
