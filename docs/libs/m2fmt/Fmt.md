# Fmt

JSON, CSV, and text table output formatting. All output is written to a caller-provided buffer -- no heap allocation anywhere.

## Why Fmt?

Generating structured output (JSON for APIs, CSV for data export, aligned tables for CLI tools) is a common need. Fmt provides three formatters behind a single buffer abstraction:

- **JSON mini-writer** with automatic comma insertion and nesting tracking.
- **CSV encoder** with RFC 4180 quoting rules.
- **Text table renderer** with auto-padded columns.

Every byte is written to a stack-allocated `Buf`, so Fmt is safe to use in constrained environments (no allocator required, no fragmentation, predictable memory usage).

## Types

### Buf

```modula2
TYPE Buf = RECORD
  data: ADDRESS;
  cap:  CARDINAL;
  pos:  CARDINAL;
END;
```

Output buffer backed by a caller-provided array. `pos` tracks how many bytes have been written. Writes beyond `cap` are silently dropped (no overflow).

## Buffer Operations

### InitBuf

```modula2
PROCEDURE InitBuf(VAR b: Buf; data: ADDRESS; cap: CARDINAL);
```

Initialise a buffer over `[data..data+cap)`. Sets `pos=0`.

### BufLen

```modula2
PROCEDURE BufLen(VAR b: Buf): CARDINAL;
```

Return the number of bytes written so far.

### BufClear

```modula2
PROCEDURE BufClear(VAR b: Buf);
```

Reset `pos` to 0. Does not zero the backing store.

## JSON Mini-Writer

The JSON procedures write syntactically correct JSON with automatic comma insertion. A nesting stack (max 16 levels) tracks whether a comma is needed before the next value.

### JsonStart / JsonEnd

```modula2
PROCEDURE JsonStart(VAR b: Buf);
PROCEDURE JsonEnd(VAR b: Buf);
```

Write `{` and `}`. Push/pop the nesting stack.

### JsonArrayStart / JsonArrayEnd

```modula2
PROCEDURE JsonArrayStart(VAR b: Buf);
PROCEDURE JsonArrayEnd(VAR b: Buf);
```

Write `[` and `]`. Push/pop the nesting stack.

### JsonKey

```modula2
PROCEDURE JsonKey(VAR b: Buf; key: ARRAY OF CHAR);
```

Write `"key":` with a leading comma if this is not the first entry at the current nesting level.

### JsonStr

```modula2
PROCEDURE JsonStr(VAR b: Buf; val: ARRAY OF CHAR);
```

Write a JSON string value with proper escaping (quotes, backslashes, control characters). Inserts comma if needed.

### JsonInt

```modula2
PROCEDURE JsonInt(VAR b: Buf; val: INTEGER);
```

Write an integer value. Inserts comma if needed.

### JsonBool

```modula2
PROCEDURE JsonBool(VAR b: Buf; val: BOOLEAN);
```

Write `true` or `false`. Inserts comma if needed.

### JsonNull

```modula2
PROCEDURE JsonNull(VAR b: Buf);
```

Write `null`. Inserts comma if needed.

## CSV Encoder

Implements RFC 4180 field encoding: fields containing commas, double quotes, or newlines are quoted, and embedded quotes are doubled.

### CsvField

```modula2
PROCEDURE CsvField(VAR b: Buf; val: ARRAY OF CHAR);
```

Write a CSV field value, automatically quoting if the value contains `,`, `"`, CR, or LF.

### CsvSep

```modula2
PROCEDURE CsvSep(VAR b: Buf);
```

Write the field separator (`,`).

### CsvNewline

```modula2
PROCEDURE CsvNewline(VAR b: Buf);
```

Write a CSV line ending (`CRLF` per RFC 4180).

## Text Table Renderer

Build a table in memory, then render it with auto-padded columns. Max 16 columns, 64 rows, 128 chars per cell.

### TableSetColumns

```modula2
PROCEDURE TableSetColumns(n: INTEGER);
```

Set the number of columns (max 16). Must be called before adding headers or rows.

### TableSetHeader

```modula2
PROCEDURE TableSetHeader(col: INTEGER; name: ARRAY OF CHAR);
```

Set the header text for column `col` (0-based).

### TableAddRow

```modula2
PROCEDURE TableAddRow(): INTEGER;
```

Add a new data row. Returns the row index for use with `TableSetCell`.

### TableSetCell

```modula2
PROCEDURE TableSetCell(row: INTEGER; col: INTEGER; value: ARRAY OF CHAR);
```

Set the value of a cell at the given row and column.

### TableRender

```modula2
PROCEDURE TableRender(VAR b: Buf);
```

Render the complete table into `b`. Column widths are computed from the maximum of header and cell widths. Output format:

```
NAME    AGE  CITY
----    ---  ----
Alice   30   Paris
Bob     25   London
```

## Example

```modula2
FROM Fmt IMPORT Buf, InitBuf, BufLen,
                JsonStart, JsonEnd, JsonKey, JsonStr, JsonInt,
                CsvField, CsvSep, CsvNewline;
FROM SYSTEM IMPORT ADR;

VAR
  raw: ARRAY [0..1023] OF CHAR;
  b: Buf;

BEGIN
  (* JSON *)
  InitBuf(b, ADR(raw), 1024);
  JsonStart(b);
    JsonKey(b, "name"); JsonStr(b, "Alice");
    JsonKey(b, "age");  JsonInt(b, 30);
  JsonEnd(b);
  (* raw[0..BufLen(b)-1] = {"name":"Alice","age":30} *)

  (* CSV *)
  BufClear(b);
  CsvField(b, "Name"); CsvSep(b); CsvField(b, "City"); CsvNewline(b);
  CsvField(b, "Alice"); CsvSep(b); CsvField(b, "Paris"); CsvNewline(b);
  (* raw[0..BufLen(b)-1] = Name,City\r\nAlice,Paris\r\n *)
END
```
