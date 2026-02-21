# RealInOut

Real number input/output module. Provides procedures for reading and
writing REAL and LONGREAL values.

## Exported Procedures

```modula2
PROCEDURE ReadReal(VAR r: REAL);
PROCEDURE WriteReal(r: REAL; width: CARDINAL);
PROCEDURE WriteRealOct(r: REAL);
PROCEDURE WriteLongReal(r: LONGREAL; width: CARDINAL);
```

## Exported Variables

```modula2
VAR Done: BOOLEAN;
```

`Done` is set to `TRUE` after a successful `ReadReal` and `FALSE`
on parse failure or end of input.

## Example

```modula2
MODULE RealDemo;
FROM RealInOut IMPORT ReadReal, WriteReal, Done;
FROM InOut IMPORT WriteString, WriteLn;
VAR x: REAL;
BEGIN
  WriteString("Enter a real number: ");
  ReadReal(x);
  IF Done THEN
    WriteString("Value: ");
    WriteReal(x, 12);
    WriteLn;
  END;
END RealDemo.
```
