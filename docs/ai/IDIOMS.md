# mx Idiomatic Patterns

Copy these patterns when generating Modula-2 code for the mx toolchain.

---

## Creating a Module Pair

**Stack.def:**
```modula-2
DEFINITION MODULE Stack;

CONST MaxSize = 128;

TYPE
  T = RECORD
    data: ARRAY [0..127] OF INTEGER;
    top: INTEGER;
  END;

PROCEDURE Init(VAR s: T);
PROCEDURE Push(VAR s: T; val: INTEGER): BOOLEAN;
PROCEDURE Pop(VAR s: T; VAR val: INTEGER): BOOLEAN;
PROCEDURE Count(VAR s: T): INTEGER;

END Stack.
```

**Stack.mod:**
```modula-2
IMPLEMENTATION MODULE Stack;

PROCEDURE Init(VAR s: T);
BEGIN
  s.top := 0
END Init;

PROCEDURE Push(VAR s: T; val: INTEGER): BOOLEAN;
BEGIN
  IF s.top >= MaxSize THEN RETURN FALSE END;
  s.data[s.top] := val;
  INC(s.top);
  RETURN TRUE
END Push;

PROCEDURE Pop(VAR s: T; VAR val: INTEGER): BOOLEAN;
BEGIN
  IF s.top = 0 THEN RETURN FALSE END;
  DEC(s.top);
  val := s.data[s.top];
  RETURN TRUE
END Pop;

PROCEDURE Count(VAR s: T): INTEGER;
BEGIN
  RETURN s.top
END Count;

END Stack.
```

---

## Importing and Using a Library

```modula-2
MODULE Main;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM ByteBuf IMPORT Buf, Init, Free, AppendByte, Len;

VAR b: Buf;
BEGIN
  Init(b, 256);
  AppendByte(b, 72);
  AppendByte(b, 101);
  WriteString("Length: ");
  WriteInt(Len(b), 1);
  WriteLn;
  Free(b)
END Main.
```

---

## String Operations

```modula-2
MODULE StringDemo;
FROM InOut IMPORT WriteString, WriteLn;
FROM Strings IMPORT Assign, Length, Concat, Pos, Copy;

VAR
  buf: ARRAY [0..255] OF CHAR;
  part: ARRAY [0..63] OF CHAR;
  pos, len: INTEGER;
BEGIN
  Assign("hello", buf);
  Concat(buf, " world", buf);
  len := Length(buf);

  (* find substring *)
  pos := Pos("world", buf);

  (* extract substring: Copy(source, startIndex, count, dest) *)
  Copy(buf, 6, 5, part);

  WriteString(buf); WriteLn;
  WriteString(part); WriteLn
END StringDemo.
```

---

## Character-by-Character Processing

```modula-2
PROCEDURE CountSpaces(s: ARRAY OF CHAR): INTEGER;
VAR i, count: INTEGER;
BEGIN
  count := 0;
  FOR i := 0 TO HIGH(s) DO
    IF s[i] = ' ' THEN INC(count)
    ELSIF s[i] = 0C THEN RETURN count
    END
  END;
  RETURN count
END CountSpaces;
```

---

## String Comparison

```modula-2
FROM Strings IMPORT CompareStr;

(* CompareStr returns: 0 = equal, <0 = less, >0 = greater *)
IF CompareStr(a, b) = 0 THEN
  WriteString("equal")
END;
```

---

## Caller-Provided Buffer Pattern

Standard pattern for procedures that produce output without heap allocation:

```modula-2
DEFINITION MODULE Util;
PROCEDURE IntToStr(val: INTEGER; VAR out: ARRAY OF CHAR; VAR len: CARDINAL);
END Util.

IMPLEMENTATION MODULE Util;

PROCEDURE IntToStr(val: INTEGER; VAR out: ARRAY OF CHAR; VAR len: CARDINAL);
VAR
  tmp: ARRAY [0..15] OF CHAR;
  i, j, n: INTEGER;
  neg: BOOLEAN;
BEGIN
  neg := val < 0;
  IF neg THEN n := -val ELSE n := val END;
  i := 0;
  REPEAT
    tmp[i] := CHR(ORD('0') + n MOD 10);
    n := n DIV 10;
    INC(i)
  UNTIL n = 0;
  IF neg THEN tmp[i] := '-'; INC(i) END;
  (* reverse into output *)
  len := i;
  FOR j := 0 TO i - 1 DO
    IF j <= HIGH(out) THEN
      out[j] := tmp[i - 1 - j]
    END
  END
END IntToStr;

END Util.
```

---

## Reading a File

```modula-2
MODULE ReadFile;
FROM InOut IMPORT WriteString, WriteLn;
FROM BinaryIO IMPORT OpenRead, Close, ReadBytes, FileSize, IsEOF;
FROM SYSTEM IMPORT ADDRESS, ADR;

VAR
  f: INTEGER;
  buf: ARRAY [0..4095] OF CHAR;
  n: INTEGER;
BEGIN
  f := OpenRead("input.txt");
  IF f < 0 THEN
    WriteString("cannot open file"); WriteLn;
    HALT
  END;
  LOOP
    n := ReadBytes(f, ADR(buf), 4096);
    IF n <= 0 THEN EXIT END;
    (* process buf[0..n-1] *)
  END;
  Close(f)
END ReadFile.
```

---

## Using m2sys for File I/O

```modula-2
MODULE SysDemo;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_fopen, m2sys_fclose, m2sys_fread_line,
                m2sys_file_exists;
FROM SYSTEM IMPORT ADR;

VAR
  fd: INTEGER;
  line: ARRAY [0..1023] OF CHAR;
  n: INTEGER;
BEGIN
  IF m2sys_file_exists(ADR("data.txt")) = 0 THEN
    WriteString("file not found"); WriteLn;
    HALT
  END;
  fd := m2sys_fopen(ADR("data.txt"), ADR("r"));
  LOOP
    n := m2sys_fread_line(fd, ADR(line), 1024);
    IF n < 0 THEN EXIT END;
    WriteString(line); WriteLn
  END;
  m2sys_fclose(fd)
END SysDemo.
```

---

## LOOP With Multiple Exit Conditions

```modula-2
PROCEDURE FindChar(s: ARRAY OF CHAR; ch: CHAR): INTEGER;
VAR i: INTEGER;
BEGIN
  i := 0;
  LOOP
    IF i > HIGH(s) THEN RETURN -1 END;
    IF s[i] = 0C THEN RETURN -1 END;
    IF s[i] = ch THEN RETURN i END;
    INC(i)
  END
END FindChar;
```

---

## Boolean Result Without IF

```modula-2
PROCEDURE IsDigit(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= '0') AND (ch <= '9')
END IsDigit;

PROCEDURE IsLetter(ch: CHAR): BOOLEAN;
BEGIN
  RETURN ((ch >= 'a') AND (ch <= 'z'))
      OR ((ch >= 'A') AND (ch <= 'Z'))
END IsLetter;
```

---

## HashMap Usage

```modula-2
FROM HashMap IMPORT Map, Bucket, Init, Put, Get, Contains;
FROM SYSTEM IMPORT ADR;

CONST TableSize = 64;

VAR
  m: Map;
  buckets: ARRAY [0..63] OF Bucket;
  val: INTEGER;

BEGIN
  Init(m, ADR(buckets), TableSize);
  Put(m, "key1", 42);
  IF Get(m, "key1", val) THEN
    WriteInt(val, 1); WriteLn
  END
END;
```

---

## CLI Argument Parsing

```modula-2
MODULE Tool;
FROM CLI IMPORT AddFlag, AddOption, Parse, HasFlag, GetOption, PrintHelp;
FROM Args IMPORT ArgCount, GetArg;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  outPath: ARRAY [0..255] OF CHAR;
BEGIN
  AddFlag("v", "verbose", "Enable verbose output");
  AddFlag("h", "help", "Show help");
  AddOption("o", "output", "Output file path");
  Parse(ArgCount(), GetArg);

  IF HasFlag("help") = 1 THEN
    PrintHelp;
    HALT
  END;
  IF GetOption("output", outPath) = 1 THEN
    WriteString("Output: "); WriteString(outPath); WriteLn
  END
END Tool.
```

---

## SET Operations

```modula-2
MODULE SetDemo;
FROM InOut IMPORT WriteString, WriteLn;

TYPE Permission = (Read, Write, Execute);
TYPE PermSet = SET OF Permission;

VAR perms: PermSet;
BEGIN
  perms := PermSet{Read, Write};
  INCL(perms, Execute);
  IF Write IN perms THEN
    WriteString("writable"); WriteLn
  END;
  EXCL(perms, Write);
  IF NOT (Write IN perms) THEN
    WriteString("no longer writable"); WriteLn
  END
END SetDemo.
```

---

## Enum with CASE Dispatch

```modula-2
TYPE Command = (CmdAdd, CmdRemove, CmdList, CmdQuit);

PROCEDURE HandleCommand(cmd: Command);
BEGIN
  CASE cmd OF
    CmdAdd:    DoAdd |
    CmdRemove: DoRemove |
    CmdList:   DoList |
    CmdQuit:   HALT
  END
END HandleCommand;
```

---

## TRY/EXCEPT Error Handling (M2+ only)

```modula-2
(* Requires --m2plus or m2plus=true in m2.toml *)
MODULE SafeIO;
FROM InOut IMPORT WriteString, WriteLn;

EXCEPTION IoError;

PROCEDURE ReadConfig(): BOOLEAN;
BEGIN
  TRY
    (* operations that may fail *)
    LoadFile("config.toml");
    RETURN TRUE
  EXCEPT IoError DO
    WriteString("config load failed"); WriteLn;
    RETURN FALSE
  FINALLY
    CleanupTempFiles
  END
END ReadConfig;

BEGIN
  IF NOT ReadConfig() THEN HALT END
END SafeIO.
```

---

## Opaque Type with Init/Free

```modula-2
(* Parser.def *)
DEFINITION MODULE Parser;
FROM SYSTEM IMPORT ADDRESS;

TYPE Handle = ADDRESS;

PROCEDURE Open(path: ARRAY OF CHAR): Handle;
PROCEDURE Close(VAR h: Handle);
PROCEDURE Next(h: Handle; VAR token: ARRAY OF CHAR): BOOLEAN;

END Parser.
```

```modula-2
(* Parser.mod *)
IMPLEMENTATION MODULE Parser;
FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  StatePtr = POINTER TO State;
  State = RECORD
    fd: INTEGER;
    pos: CARDINAL;
    buf: ARRAY [0..4095] OF CHAR;
  END;

PROCEDURE Open(path: ARRAY OF CHAR): Handle;
VAR p: StatePtr;
BEGIN
  NEW(p);
  p^.pos := 0;
  RETURN p
END Open;

PROCEDURE Close(VAR h: Handle);
VAR p: StatePtr;
BEGIN
  p := h;
  DISPOSE(p);
  h := NIL
END Close;

PROCEDURE Next(h: Handle; VAR token: ARRAY OF CHAR): BOOLEAN;
VAR p: StatePtr;
BEGIN
  p := h;
  (* ... *)
  RETURN FALSE
END Next;

END Parser.
```
