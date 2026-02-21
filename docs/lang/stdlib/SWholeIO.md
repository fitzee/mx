# SWholeIO

ISO-style whole number input/output module. Provides formatted
reading and writing of INTEGER and CARDINAL values following
ISO 10514 conventions.

## Exported Procedures

```modula2
PROCEDURE ReadInt(VAR n: INTEGER);
PROCEDURE WriteInt(n: INTEGER; width: CARDINAL);
PROCEDURE ReadCard(VAR n: CARDINAL);
PROCEDURE WriteCard(n: CARDINAL; width: CARDINAL);
```

## Notes

- `WriteInt` and `WriteCard` right-justify the number in a field
  of the given `width`. If `width` is 0, no padding is added.
- `ReadInt` parses a signed decimal integer from standard input.
- `ReadCard` parses an unsigned decimal cardinal from standard input.
- This module mirrors the ISO Modula-2 SWholeIO interface.

## Example

```modula2
MODULE SWholeDemo;
FROM SWholeIO IMPORT ReadInt, WriteInt, WriteCard;
FROM STextIO IMPORT WriteString, WriteLn;
VAR n: INTEGER;
BEGIN
  WriteString("Enter an integer: ");
  ReadInt(n);
  WriteString("Value: ");
  WriteInt(n, 8);
  WriteLn;
END SWholeDemo.
```
