# SRealIO

REAL (single-precision floating-point) I/O following the ISO 10514 Modula-2 standard. Provides three output formats: scientific notation, fixed-point, and general-purpose. This is the ISO-style equivalent of `RealInOut`.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

### Output

```modula2
PROCEDURE WriteFloat(r: REAL; sigFigs: CARDINAL; w: CARDINAL);
```
Write `r` in scientific notation (e.g., `1.414E+00`) with `sigFigs` significant figures, right-justified in a field of width `w`. Use this when you need consistent exponential notation.

```modula2
PROCEDURE WriteFixed(r: REAL; place: INTEGER; w: CARDINAL);
```
Write `r` in fixed-point format with `place` digits after the decimal point, right-justified in width `w`. Use this for tabular output where you want a consistent number of decimal places.

Example: `WriteFixed(3.14159, 2, 10)` prints `"      3.14"`.

```modula2
PROCEDURE WriteReal(r: REAL; w: CARDINAL);
```
Write `r` in general format (like C's `%g`), right-justified in width `w`. The format is automatically chosen: fixed-point for moderate values, scientific for very large or very small values.

### Input

```modula2
PROCEDURE ReadReal(VAR r: REAL);
```
Read a REAL value from stdin. Accepts decimal notation (`3.14`), scientific notation (`1.0E-3`), and integer notation (`42`, which is read as `42.0`).

## Example

```modula2
MODULE SRealIODemo;
FROM STextIO IMPORT WriteString, WriteLn;
FROM SRealIO IMPORT WriteFloat, WriteFixed, WriteReal;
FROM MathLib IMPORT sqrt;

VAR r: REAL;

BEGIN
  r := sqrt(2.0);

  WriteString("General:    ");
  WriteReal(r, 12); WriteLn;

  WriteString("Fixed 4dp:  ");
  WriteFixed(r, 4, 12); WriteLn;

  WriteString("Scientific: ");
  WriteFloat(r, 6, 12); WriteLn
END SRealIODemo.
```
