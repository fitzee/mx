# Json

## Why
Provides a SAX-style streaming JSON tokenizer that operates on caller-provided buffers. Zero heap allocation, no OS syscalls, no C bridge. Pure Modula-2.

## Types

- **TokenKind** -- `JNull`, `JTrue`, `JFalse`, `JNumber`, `JString`, `JArrayStart`, `JArrayEnd`, `JObjectStart`, `JObjectEnd`, `JColon`, `JComma`, `JError`, `JEnd`.
- **Token** -- Record with `kind: TokenKind`, `start: CARDINAL` (byte offset), `len: CARDINAL` (byte length).
- **Parser** -- Parser state holding source pointer, position, and error buffer.

## Procedures

### Lifecycle

- `PROCEDURE Init(VAR p: Parser; src: ADDRESS; srcLen: CARDINAL)`
  Initialise a parser over a buffer. Does not copy the source.

### Tokenising

- `PROCEDURE Next(VAR p: Parser; VAR tok: Token): BOOLEAN`
  Pull the next token. Returns FALSE on error or end of input.

- `PROCEDURE Skip(VAR p: Parser)`
  Skip a complete JSON value, including nested arrays and objects.

### Value Extraction

- `PROCEDURE GetString(VAR p: Parser; VAR tok: Token; VAR buf: ARRAY OF CHAR): BOOLEAN`
  Extract a string token into buf with JSON escape processing (`\n`, `\t`, `\"`, `\\`, `\uXXXX`).

- `PROCEDURE GetInteger(VAR p: Parser; VAR tok: Token; VAR val: INTEGER): BOOLEAN`
  Parse a number token as a 32-bit integer.

- `PROCEDURE GetLong(VAR p: Parser; VAR tok: Token; VAR val: LONGINT): BOOLEAN`
  Parse a number token as a 64-bit integer.

- `PROCEDURE GetReal(VAR p: Parser; VAR tok: Token; VAR val: REAL): BOOLEAN`
  Parse a number token as a floating-point value.

### Error Handling

- `PROCEDURE GetError(VAR p: Parser; VAR buf: ARRAY OF CHAR)`
  Copy the current error message into buf.

## Example

```modula2
MODULE JsonDemo;

FROM SYSTEM IMPORT ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Json IMPORT Parser, Token, TokenKind,
                 Init, Next, GetString, GetInteger;

VAR
  p: Parser;
  tok: Token;
  buf: ARRAY [0..255] OF CHAR;
  n: INTEGER;
  src: ARRAY [0..63] OF CHAR;

BEGIN
  src := '{"name":"mx","version":1}';
  Init(p, ADR(src), 26);

  WHILE Next(p, tok) DO
    IF tok.kind = JString THEN
      IF GetString(p, tok, buf) THEN
        WriteString(buf); WriteLn
      END
    ELSIF tok.kind = JNumber THEN
      IF GetInteger(p, tok, n) THEN
        WriteInt(n, 0); WriteLn
      END
    END
  END
END JsonDemo.
```
