# RealInOut

Floating-point I/O for PIM4 programs. Provides procedures to read and write REAL (single-precision) values in various output formats. For LONGREAL (double-precision) output, use `SLongIO` instead.

Available in PIM4 mode (the default). No special flags needed.

## Variables

| Variable | Type | Description |
|----------|------|-------------|
| `Done` | `BOOLEAN` | Set by `ReadReal`. `TRUE` if a valid floating-point number was read from stdin, `FALSE` otherwise. |

## Procedures

```modula2
PROCEDURE ReadReal(VAR r: REAL);
```
Read a floating-point number from stdin and store it in `r`. Accepts standard decimal notation (e.g., `3.14`, `-0.5`, `1.0E10`). Sets `Done` to `FALSE` if the input cannot be parsed as a number.

```modula2
PROCEDURE WriteReal(r: REAL; w: CARDINAL);
```
Write `r` in general format (similar to C's `%g`), right-justified in a field of width `w`. The general format automatically chooses between fixed and scientific notation based on the magnitude of the number. Use `w=1` for minimum-width output.

```modula2
PROCEDURE WriteFixPt(r: REAL; w: CARDINAL; d: CARDINAL);
```
Write `r` in fixed-point format with exactly `d` digits after the decimal point, right-justified in width `w`. This is useful when you need a consistent number of decimal places (e.g., for currency or tabular output).

Example: `WriteFixPt(3.14159, 10, 2)` prints `"      3.14"`.

```modula2
PROCEDURE WriteRealOct(r: REAL);
```
Write `r` as a hexadecimal representation of its IEEE 754 bit pattern. This is a debugging aid, not a human-readable format.

## Example

```modula2
MODULE RealDemo;
FROM InOut IMPORT WriteString, WriteLn;
FROM RealInOut IMPORT WriteReal, WriteFixPt;
FROM MathLib IMPORT sqrt;

BEGIN
  WriteString("General:    ");
  WriteReal(sqrt(2.0), 12); WriteLn;

  WriteString("Fixed 2dp:  ");
  WriteFixPt(sqrt(2.0), 12, 2); WriteLn;

  WriteString("Fixed 6dp:  ");
  WriteFixPt(sqrt(2.0), 12, 6); WriteLn
END RealDemo.
```
