# SWholeIO

Integer and cardinal I/O following the ISO 10514 Modula-2 standard. Reads and writes whole numbers (integers and cardinals) with field-width formatting.

This is the ISO-style equivalent of the `WriteInt`/`WriteCard`/`ReadInt`/`ReadCard` procedures in InOut. The functionality is the same -- the difference is naming convention and module organization. You can use either; most PIM4 programs use InOut.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

### Output

```modula2
PROCEDURE WriteInt(n: INTEGER; w: CARDINAL);
```
Write a signed integer to stdout, right-justified in a field of width `w`. If the number takes fewer characters than `w`, spaces are padded on the left. If it takes more, the full number is printed. Use `w=1` for minimum-width output.

```modula2
PROCEDURE WriteCard(n: CARDINAL; w: CARDINAL);
```
Write an unsigned cardinal to stdout, right-justified in width `w`. Same padding rules as `WriteInt`.

### Input

```modula2
PROCEDURE ReadInt(VAR n: INTEGER);
```
Read a signed integer from stdin. Skips leading whitespace, then reads an optional sign and decimal digits.

```modula2
PROCEDURE ReadCard(VAR n: CARDINAL);
```
Read an unsigned cardinal from stdin. Skips leading whitespace, then reads decimal digits.

## Example

```modula2
MODULE SWholeIODemo;
FROM STextIO IMPORT WriteString, WriteLn;
FROM SWholeIO IMPORT WriteInt, WriteCard;
BEGIN
  WriteString("Signed:   ");
  WriteInt(-42, 8); WriteLn;

  WriteString("Unsigned: ");
  WriteCard(1000, 8); WriteLn
END SWholeIODemo.
```

Output:
```
Signed:       -42
Unsigned:     1000
```
