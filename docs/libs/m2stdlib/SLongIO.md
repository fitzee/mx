# SLongIO

LONGREAL (double-precision floating-point) I/O following the ISO 10514 Modula-2 standard. Same output formats as SRealIO (scientific, fixed-point, general) but for double-precision values.

Use this module when you need the extra precision of LONGREAL (64-bit `double` in C). For REAL (32-bit `float`), use SRealIO or RealInOut instead.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

### Output

```modula2
PROCEDURE WriteFloat(r: LONGREAL; sigFigs: CARDINAL; w: CARDINAL);
```
Write `r` in scientific notation with `sigFigs` significant figures, right-justified in width `w`.

```modula2
PROCEDURE WriteFixed(r: LONGREAL; place: INTEGER; w: CARDINAL);
```
Write `r` in fixed-point format with `place` digits after the decimal point, right-justified in width `w`.

```modula2
PROCEDURE WriteLongReal(r: LONGREAL; w: CARDINAL);
```
Write `r` in general format (automatic choice of fixed or scientific), right-justified in width `w`.

### Input

```modula2
PROCEDURE ReadLongReal(VAR r: LONGREAL);
```
Read a LONGREAL value from stdin. Accepts the same input formats as `SRealIO.ReadReal` but stores the result with double precision.

## Example

```modula2
MODULE SLongIODemo;
FROM STextIO IMPORT WriteString, WriteLn;
FROM SLongIO IMPORT WriteLongReal, WriteFixed;

VAR pi: LONGREAL;

BEGIN
  pi := 3.14159265358979;

  WriteString("General:    ");
  WriteLongReal(pi, 20); WriteLn;

  WriteString("Fixed 10dp: ");
  WriteFixed(pi, 10, 20); WriteLn
END SLongIODemo.
```
