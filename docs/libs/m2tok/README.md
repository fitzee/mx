# Tokenizer

## Why
Provides a language-agnostic source code tokenizer that classifies input into identifiers, operators, and shebang lines while stripping string literals and comments -- all without heap allocation.

## Types

- **TokenKind** -- Classification of a token: `Ident`, `Operator`, `Shebang`, `EndOfInput`.
- **Token** -- A single token record containing:
  - `start: CARDINAL` -- Byte offset into the source buffer.
  - `len: CARDINAL` -- Length of the token in bytes.
  - `kind: TokenKind` -- The token classification.
- **State** -- Scanner state containing:
  - `buf: ADDRESS` -- Pointer to the source byte buffer.
  - `blen: CARDINAL` -- Total buffer length.
  - `pos: CARDINAL` -- Current scan position.

## Procedures

- `PROCEDURE Init(VAR s: State; buf: ADDRESS; len: CARDINAL)`
  Initialise tokenizer state over a byte buffer.

- `PROCEDURE Next(VAR s: State; VAR t: Token): BOOLEAN`
  Advance to the next token. Returns TRUE and fills `t` with the token, or returns FALSE at end-of-input.

- `PROCEDURE CopyToken(VAR s: State; VAR t: Token; VAR out: ARRAY OF CHAR)`
  Copy the text of token `t` into `out`, NUL-terminated. Truncates if `out` is too small.

## Example

```modula2
MODULE TokenizerExample;

FROM SYSTEM IMPORT ADR;
FROM Tokenizer IMPORT State, Token, TokenKind, Init, Next, CopyToken;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  s: State;
  t: Token;
  word: ARRAY [0..63] OF CHAR;
  source: ARRAY [0..127] OF CHAR;

BEGIN
  source := "func main() { x := 42; }";
  Init(s, ADR(source), 25);

  WHILE Next(s, t) DO
    CopyToken(s, t, word);
    CASE t.kind OF
      Ident:
        WriteString("IDENT: ");
        WriteString(word);
        WriteLn;
    | Operator:
        WriteString("  OP: ");
        WriteString(word);
        WriteLn;
    | Shebang:
        WriteString("SHEBANG: ");
        WriteString(word);
        WriteLn;
    END;
  END;
END TokenizerExample.
```

Output:
```
IDENT: func
IDENT: main
  OP: (
  OP: )
  OP: {
IDENT: x
  OP: :
  OP: =
IDENT: 42
  OP: ;
  OP: }
```
