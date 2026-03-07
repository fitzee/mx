# Regex

## Why
Provides regular expression matching via the system POSIX regex.h. Uses extended regex syntax (REG_EXTENDED). No extra libraries needed beyond what the OS provides.

## Types

- **Regex** (ADDRESS) -- Opaque compiled pattern handle.
- **Match** -- Record with `start: CARDINAL` (byte offset) and `len: CARDINAL` (match length).
- **Status** -- `Ok`, `NoMatch`, `BadPattern`, `Error`.

## Constants

- `MaxMatches = 32` -- Maximum number of matches returned by FindAll.
- `MaxErrorLen = 256` -- Maximum error message length.

## Procedures

### Lifecycle

- `PROCEDURE Compile(pattern: ARRAY OF CHAR; VAR re: Regex): Status`
  Compile a regex pattern. Returns BadPattern if the pattern is invalid.

- `PROCEDURE Free(VAR re: Regex)`
  Free a compiled pattern.

### Matching

- `PROCEDURE Test(re: Regex; text: ARRAY OF CHAR): BOOLEAN`
  Test whether text matches the pattern. Returns TRUE/FALSE.

- `PROCEDURE Find(re: Regex; text: ARRAY OF CHAR; VAR m: Match): Status`
  Find the first match in text. Fills m with the match position.

- `PROCEDURE FindAll(re: Regex; text: ARRAY OF CHAR; VAR matches: ARRAY OF Match; maxMatches: CARDINAL; VAR count: CARDINAL): Status`
  Find all non-overlapping matches. Returns up to maxMatches results.

### Error Handling

- `PROCEDURE GetError(VAR buf: ARRAY OF CHAR)`
  Copy the last error message into buf.

## Example

```modula2
MODULE RegexDemo;

FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM Regex IMPORT Regex, Match, Status, Compile, Find, Free, Ok;

VAR
  re: Regex;
  m: Match;
  s: Status;

BEGIN
  s := Compile("[0-9]+", re);
  IF s = Ok THEN
    s := Find(re, "version 42 released", m);
    IF s = Ok THEN
      WriteString("found at offset ");
      WriteCard(m.start, 0);
      WriteString(", length ");
      WriteCard(m.len, 0);
      WriteLn
    END;
    Free(re)
  END
END RegexDemo.
```
