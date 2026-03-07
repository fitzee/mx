# Glob

Gitignore-grade glob pattern matching with support for `*`, `?`, character classes, and `**` directory wildcards. No heap allocation.

## Why Glob?

File-matching patterns appear everywhere: `.gitignore` rules, build system file filters, package exclude lists. Glob implements the full gitignore pattern syntax so you can match paths against patterns like `*.mod`, `src/**/*.def`, or `[!.]*.txt` without shelling out to an external tool.

The matching engine uses an iterative algorithm with a backtrack stack (max 32 entries for `**` expansion), so it handles deeply nested directory patterns without recursion and without heap allocation.

## Pattern Syntax

| Pattern | Matches | Does NOT match across |
|---------|---------|----------------------|
| `*` | Zero or more non-`/` characters | `/` path separator |
| `?` | Exactly one non-`/` character | `/` path separator |
| `[abc]` | Any one of `a`, `b`, `c` | |
| `[a-z]` | Any character in range `a`..`z` | |
| `[!x]` | Any character except `x` | |
| `**/` | Zero or more directory components | |
| `/**` | Everything below a directory | |
| `a/**/b` | Zero or more intermediate directories | |

Both `*` and `?` never match `/`, which preserves directory boundaries.

## Procedures

### Match

```modula2
PROCEDURE Match(pattern: ARRAY OF CHAR;
                text: ARRAY OF CHAR): BOOLEAN;
```

Full glob match: returns `TRUE` if the **entire** text matches the pattern. This is not a substring search -- the pattern must account for the whole input.

### IsNegated

```modula2
PROCEDURE IsNegated(pattern: ARRAY OF CHAR): BOOLEAN;
```

Returns `TRUE` if the pattern starts with `!` (gitignore negation prefix). A negated pattern re-includes files excluded by earlier rules.

### IsAnchored

```modula2
PROCEDURE IsAnchored(pattern: ARRAY OF CHAR): BOOLEAN;
```

Returns `TRUE` if the pattern starts with `/` (anchored to the root). Anchored patterns only match relative to the repository root, not in subdirectories.

### HasPathSep

```modula2
PROCEDURE HasPathSep(pattern: ARRAY OF CHAR): BOOLEAN;
```

Returns `TRUE` if the pattern contains `/` anywhere. Patterns without path separators match against the basename only; patterns with separators match against the full path.

### StripNegation

```modula2
PROCEDURE StripNegation(pattern: ARRAY OF CHAR;
                        VAR out: ARRAY OF CHAR);
```

Copy the pattern into `out` with the leading `!` removed. If the pattern is not negated, copies it unchanged.

### StripAnchor

```modula2
PROCEDURE StripAnchor(pattern: ARRAY OF CHAR;
                      VAR out: ARRAY OF CHAR);
```

Copy the pattern into `out` with the leading `/` removed. If the pattern is not anchored, copies it unchanged.

## Example

```modula2
FROM Glob IMPORT Match, IsNegated, HasPathSep;
FROM InOut IMPORT WriteString, WriteLn;

BEGIN
  IF Match("*.mod", "Hello.mod") THEN
    WriteString("matched .mod file"); WriteLn
  END;

  IF Match("src/**/*.def", "src/utils/Strings.def") THEN
    WriteString("matched nested .def"); WriteLn
  END;

  IF Match("[!.]*", "README.md") THEN
    WriteString("non-dotfile"); WriteLn
  END;

  IF IsNegated("!build/") THEN
    WriteString("negated rule"); WriteLn
  END;

  IF HasPathSep("src/*.mod") THEN
    WriteString("path pattern — match full path"); WriteLn
  ELSE
    WriteString("basename pattern — match filename only"); WriteLn
  END
END
```
