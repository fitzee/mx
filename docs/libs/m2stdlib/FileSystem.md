# FileSystem

Basic character-level file I/O with an opaque `File` handle. This is the PIM4 standard file module -- it reads and writes one character at a time. For reading/writing raw bytes, blocks of data, or seeking within a file, use `BinaryIO` instead.

Available in PIM4 mode (the default). No special flags needed.

## Types

| Type | Description |
|------|-------------|
| `File` | Opaque file handle (internally an `ADDRESS`). Created by `Lookup`, released by `Close`. |

## Variables

| Variable | Type | Description |
|----------|------|-------------|
| `Done` | `BOOLEAN` | Set after every operation. `TRUE` if it succeeded, `FALSE` on error (file not found, EOF, etc.). |

## Procedures

```modula2
PROCEDURE Lookup(VAR f: File; name: ARRAY OF CHAR; newFile: BOOLEAN);
```
Open a file by name. If `newFile` is `TRUE` and the file does not exist, it is created. If `newFile` is `FALSE` and the file does not exist, `Done` is set to `FALSE`. On success, `f` receives a valid file handle.

The parameter name `newFile` is a PIM4 convention meaning "create if absent" -- it does not mean "always create a new file".

```modula2
PROCEDURE Close(VAR f: File);
```
Close the file and release the handle. After this call, `f` should not be used.

```modula2
PROCEDURE ReadChar(VAR f: File; VAR ch: CHAR);
```
Read the next character from the file into `ch`. At end-of-file, `Done` is set to `FALSE`.

```modula2
PROCEDURE WriteChar(VAR f: File; ch: CHAR);
```
Write one character to the file.

## Example

```modula2
MODULE FileDemo;
FROM InOut IMPORT WriteString, WriteLn;
FROM FileSystem IMPORT File, Lookup, Close, ReadChar, WriteChar, Done;

VAR
  f: File;
  ch: CHAR;

BEGIN
  (* Write a file *)
  Lookup(f, "test.txt", TRUE);
  IF Done THEN
    WriteChar(f, 'H');
    WriteChar(f, 'i');
    Close(f)
  END;

  (* Read it back *)
  Lookup(f, "test.txt", FALSE);
  IF Done THEN
    LOOP
      ReadChar(f, ch);
      IF NOT Done THEN EXIT END;
      InOut.Write(ch)
    END;
    Close(f);
    WriteLn
  END
END FileDemo.
```
