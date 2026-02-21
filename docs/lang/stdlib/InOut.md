# InOut

Standard input/output module for reading and writing basic types
to the terminal. This is the most commonly used I/O module in
PIM4 Modula-2 programs.

## Exported Procedures

```modula2
PROCEDURE WriteString(s: ARRAY OF CHAR);
PROCEDURE WriteLn;
PROCEDURE WriteInt(n: INTEGER; width: CARDINAL);
PROCEDURE WriteCard(n: CARDINAL; width: CARDINAL);
PROCEDURE WriteChar(ch: CHAR);
PROCEDURE ReadString(VAR s: ARRAY OF CHAR);
PROCEDURE ReadInt(VAR n: INTEGER);
PROCEDURE ReadCard(VAR n: CARDINAL);
PROCEDURE ReadChar(VAR ch: CHAR);
PROCEDURE Read(VAR ch: CHAR);
PROCEDURE Write(ch: CHAR);
```

## Exported Variables

```modula2
VAR Done: BOOLEAN;
```

`Done` is set to `TRUE` after a successful read operation and `FALSE`
if the read failed (e.g., end of input or invalid format).

## Example

```modula2
MODULE Hello;
FROM InOut IMPORT WriteString, WriteLn, ReadInt, WriteInt, Done;
VAR n: INTEGER;
BEGIN
  WriteString("Enter a number: ");
  ReadInt(n);
  IF Done THEN
    WriteString("You entered: ");
    WriteInt(n, 0);
    WriteLn;
  END;
END Hello.
```
