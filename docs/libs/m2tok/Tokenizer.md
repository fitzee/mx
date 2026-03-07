# Tokenizer

Language-agnostic source code tokenizer for file classification. Strips string literals, line comments, and block comments, then yields identifiers and punctuation operators from a byte buffer. No heap allocation.

## Why Tokenizer?

Language classifiers (Bayesian, TF-IDF, nearest-neighbour) need a bag of tokens extracted from source files. Raw byte scanning is too noisy -- string contents and comments add vocabulary that does not help distinguish languages. Tokenizer handles the common syntax patterns shared across most programming languages (C-style comments, quoted strings, shebangs) so the classifier receives clean identifier and operator tokens.

The tokenizer operates on an `ADDRESS` + length buffer, making it composable with m2sys file I/O and m2bytes buffers without copying.

## Types

### TokenKind

```modula2
TYPE TokenKind = (Ident, Operator, Shebang, EndOfInput);
```

| Value | Meaning |
|-------|---------|
| `Ident` | Identifier: a run of letters, digits, and underscores |
| `Operator` | Single punctuation character (anything not whitespace, not part of a skipped construct, and not alphanumeric) |
| `Shebang` | A `#!` line at position 0 (e.g., `#!/usr/bin/env python`) |
| `EndOfInput` | Returned implicitly when `Next` returns FALSE |

### Token

```modula2
TYPE Token = RECORD
  start: CARDINAL;
  len:   CARDINAL;
  kind:  TokenKind;
END;
```

A token's position within the source buffer. Use `CopyToken` to extract the text.

### State

```modula2
TYPE State = RECORD
  buf:  ADDRESS;
  blen: CARDINAL;
  pos:  CARDINAL;
END;
```

Tokenizer state. `buf` and `blen` are the source buffer; `pos` is the current scan position.

## Procedures

### Init

```modula2
PROCEDURE Init(VAR s: State; buf: ADDRESS; len: CARDINAL);
```

Initialise the tokenizer over a byte buffer. Sets `pos` to 0.

### Next

```modula2
PROCEDURE Next(VAR s: State; VAR t: Token): BOOLEAN;
```

Advance to the next token. Returns TRUE and fills `t` with the token's position, length, and kind. Returns FALSE at end-of-input.

During scanning, the following constructs are silently skipped:

- **Double-quoted strings** (`"..."`) with backslash escape handling
- **Single-quoted strings** (`'...'`) with backslash escape handling
- **Line comments** starting with `//` or `#` (through end of line)
- **Block comments** (`/* ... */`) with nesting support

A `#!` at position 0 is recognised as a shebang line and returned as a `Shebang` token rather than being skipped as a comment.

### CopyToken

```modula2
PROCEDURE CopyToken(VAR s: State; VAR t: Token; VAR out: ARRAY OF CHAR);
```

Copy the text of token `t` into `out`, NUL-terminated. If `out` is too small, the text is truncated to `HIGH(out) - 1` characters plus a NUL terminator.

## Skipping Rules

| Construct | Trigger | End condition |
|-----------|---------|---------------|
| Double-quoted string | `"` | Closing `"` (backslash escapes skipped) |
| Single-quoted string | `'` | Closing `'` (backslash escapes skipped) |
| Line comment (`//`) | `//` | Newline or end-of-buffer |
| Line comment (`#`) | `#` (not at pos 0 with `!`) | Newline or end-of-buffer |
| Block comment | `/*` | Matching `*/` (nesting supported) |
| Shebang | `#!` at pos 0 | Newline or end-of-buffer (returned as token) |

## Example

```modula2
FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Length;
FROM Tokenizer IMPORT TokenKind, Token, State, Init, Next, CopyToken;

VAR
  src: ARRAY [0..127] OF CHAR;
  s: State;
  t: Token;
  word: ARRAY [0..63] OF CHAR;

src := 'int main() { return 0; /* done */ }';
Init(s, ADR(src), Length(src));
WHILE Next(s, t) DO
  CopyToken(s, t, word);
  (* yields: int, main, (, ), {, return, 0, ;, } *)
END;
```
