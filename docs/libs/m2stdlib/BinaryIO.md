# BinaryIO

Binary file I/O with a handle-based file table. Unlike `FileSystem` (which reads/writes one character at a time), BinaryIO works with raw bytes and byte blocks, supports seeking to arbitrary positions, and can query file size. Use this module when you need to read binary data, process files in chunks, or random-access file contents.

File handles are `CARDINAL` values (small integers) rather than opaque pointers. The module manages an internal table of open files.

Available in PIM4 mode (the default). No special flags needed.

## Variables

| Variable | Type | Description |
|----------|------|-------------|
| `Done` | `BOOLEAN` | Set by `OpenRead` and `OpenWrite`. `TRUE` if the file was opened successfully. |

## Procedures

### Opening and Closing

```modula2
PROCEDURE OpenRead(name: ARRAY OF CHAR; VAR fh: CARDINAL);
```
Open a file for reading. On success, `fh` receives a handle that identifies the file for subsequent operations, and `Done` is set to `TRUE`. On failure (file not found), `Done` is `FALSE`.

```modula2
PROCEDURE OpenWrite(name: ARRAY OF CHAR; VAR fh: CARDINAL);
```
Open a file for writing. Creates the file if it does not exist, truncates it if it does. On success, `fh` receives the handle.

```modula2
PROCEDURE Close(fh: CARDINAL);
```
Close the file. After this call, the handle is invalid and should not be reused.

### Byte-Level I/O

```modula2
PROCEDURE ReadByte(fh: CARDINAL; VAR b: CARDINAL);
```
Read one byte from the file. The byte value (0..255) is stored in `b`. At EOF, `b` is undefined and you should check with `IsEOF`.

```modula2
PROCEDURE WriteByte(fh: CARDINAL; b: CARDINAL);
```
Write one byte to the file. Only the low 8 bits of `b` are written.

### Block I/O

```modula2
PROCEDURE ReadBytes(fh: CARDINAL; VAR buf: ARRAY OF CHAR; n: CARDINAL;
                    VAR actual: CARDINAL);
```
Read up to `n` bytes from the file into `buf`. `actual` receives the number of bytes actually read (may be less than `n` at EOF).

```modula2
PROCEDURE WriteBytes(fh: CARDINAL; buf: ARRAY OF CHAR; n: CARDINAL);
```
Write `n` bytes from `buf` to the file.

### Position and Size

```modula2
PROCEDURE FileSize(fh: CARDINAL; VAR size: CARDINAL);
```
Get the total size of the file in bytes.

```modula2
PROCEDURE Seek(fh: CARDINAL; pos: CARDINAL);
```
Move the read/write position to byte offset `pos` (0-based from the start of the file).

```modula2
PROCEDURE Tell(fh: CARDINAL; VAR pos: CARDINAL);
```
Get the current read/write position.

```modula2
PROCEDURE IsEOF(fh: CARDINAL): BOOLEAN;
```
Return `TRUE` if the current position is at or past the end of the file.

## Example

```modula2
MODULE BinaryDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM BinaryIO IMPORT OpenRead, OpenWrite, Close, ReadBytes,
                     WriteBytes, FileSize, Done;

VAR
  fh: CARDINAL;
  buf: ARRAY [0..1023] OF CHAR;
  actual, size: CARDINAL;

BEGIN
  (* Write some data *)
  OpenWrite("test.bin", fh);
  IF Done THEN
    buf[0] := CHR(72);  (* 'H' *)
    buf[1] := CHR(101); (* 'e' *)
    buf[2] := CHR(108); (* 'l' *)
    buf[3] := CHR(108); (* 'l' *)
    buf[4] := CHR(111); (* 'o' *)
    WriteBytes(fh, buf, 5);
    Close(fh)
  END;

  (* Read it back *)
  OpenRead("test.bin", fh);
  IF Done THEN
    FileSize(fh, size);
    WriteString("File size: ");
    WriteInt(INTEGER(size), 1);
    WriteString(" bytes"); WriteLn;

    ReadBytes(fh, buf, 1024, actual);
    WriteString("Read ");
    WriteInt(INTEGER(actual), 1);
    WriteString(" bytes"); WriteLn;
    Close(fh)
  END
END BinaryDemo.
```
