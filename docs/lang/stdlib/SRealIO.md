# SRealIO

ISO-style real number input/output module. Provides formatted
reading and writing of REAL values following ISO 10514 conventions.

## Exported Procedures

```modula2
PROCEDURE ReadReal(VAR r: REAL);
PROCEDURE WriteFloat(r: REAL; sigFigs: CARDINAL; width: CARDINAL);
PROCEDURE WriteFixed(r: REAL; decPlaces: INTEGER; width: CARDINAL);
PROCEDURE WriteEng(r: REAL; sigFigs: CARDINAL; width: CARDINAL);
```

## Notes

- `WriteFloat` outputs in scientific notation with `sigFigs`
  significant figures, right-justified in `width` characters.
- `WriteFixed` outputs in fixed-point notation with `decPlaces`
  digits after the decimal point.
- `WriteEng` outputs in engineering notation (exponent is a
  multiple of 3) with `sigFigs` significant figures.
- This module mirrors the ISO Modula-2 SRealIO interface.

## Example

```modula2
MODULE SRealDemo;
FROM SRealIO IMPORT ReadReal, WriteFloat, WriteFixed;
FROM STextIO IMPORT WriteString, WriteLn;
VAR x: REAL;
BEGIN
  WriteString("Enter a real: ");
  ReadReal(x);
  WriteString("Float:  "); WriteFloat(x, 6, 14); WriteLn;
  WriteString("Fixed:  "); WriteFixed(x, 3, 10); WriteLn;
END SRealDemo.
```
