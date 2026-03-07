# Path

Pure UNIX-style path string manipulation. Normalize, split, join, compute relative paths, extract extensions, and glob-match basenames -- all without heap allocation.

## Why Path?

Path manipulation is needed everywhere: resolving imports, computing output paths, matching file patterns. Path provides the core operations on `/`-separated path strings using only caller-provided buffers. There are no OS syscalls (no `realpath`, no `stat`), so Path works identically in all environments and is safe to use during compilation.

Internally, paths are split into segments (max 64 segments, 256 chars each) for normalization and relative path computation.

## Procedures

### Normalize

```modula2
PROCEDURE Normalize(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
```

Collapse `.` and `..` segments, strip trailing `/`, and merge consecutive `//`. Preserves leading `/` for absolute paths. Cannot navigate above root: `/../a` normalizes to `/a`. Special cases: `"."` stays `"."`, `".."` stays `".."`, `"/"` stays `"/"`.

### Extension

```modula2
PROCEDURE Extension(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
```

Extract the file extension including the leading dot. Returns `""` if there is no extension. Dotfiles are handled correctly: `.gitignore` has no extension (the leading dot is part of the name, not an extension separator).

| Input | Result |
|-------|--------|
| `"foo.mod"` | `".mod"` |
| `"foo.tar.gz"` | `".gz"` |
| `"foo"` | `""` |
| `".gitignore"` | `""` |

### StripExt

```modula2
PROCEDURE StripExt(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
```

Remove the extension from the path. `"foo.mod"` becomes `"foo"`, `"/a/b.txt"` becomes `"/a/b"`, `"foo"` is unchanged.

### IsAbsolute

```modula2
PROCEDURE IsAbsolute(path: ARRAY OF CHAR): BOOLEAN;
```

Returns `TRUE` if the path starts with `/`.

### Split

```modula2
PROCEDURE Split(path: ARRAY OF CHAR;
                VAR dir: ARRAY OF CHAR; VAR base: ARRAY OF CHAR);
```

Split a path into its directory and basename components.

| Input | dir | base |
|-------|-----|------|
| `"/a/b/c"` | `"/a/b"` | `"c"` |
| `"foo"` | `""` | `"foo"` |
| `"/"` | `"/"` | `""` |
| `"/a"` | `"/"` | `"a"` |

### RelativeTo

```modula2
PROCEDURE RelativeTo(base: ARRAY OF CHAR; target: ARRAY OF CHAR;
                     VAR out: ARRAY OF CHAR);
```

Compute a relative path from `base` to `target`. Both paths are normalized first. Uses `..` to navigate up from `base` to the common ancestor, then descends to `target`.

| base | target | result |
|------|--------|--------|
| `"/a/b"` | `"/a/c/d"` | `"../c/d"` |
| `"/a/b"` | `"/a/b"` | `"."` |
| `"/a/b/c"` | `"/a"` | `"../.."` |

### Join

```modula2
PROCEDURE Join(a: ARRAY OF CHAR; b: ARRAY OF CHAR;
               VAR out: ARRAY OF CHAR);
```

Join two path components with `/`. If `b` is absolute, `b` wins (e.g., `Join("a", "/b", out)` produces `"/b"`). Avoids double slashes: `Join("a/", "b", out)` produces `"a/b"`. Empty `a`: `Join("", "b", out)` produces `"b"`.

### Match

```modula2
PROCEDURE Match(path: ARRAY OF CHAR; pattern: ARRAY OF CHAR): BOOLEAN;
```

Simple glob match on the **basename** of `path`. `*` matches zero or more non-`/` characters, `?` matches exactly one non-`/` character. The pattern is matched against the basename only, not the full path.

## Example

```modula2
FROM Path IMPORT Normalize, Split, Join, Extension, RelativeTo, Match;
FROM InOut IMPORT WriteString, WriteLn;

VAR dir, base, ext, out: ARRAY [0..255] OF CHAR;

BEGIN
  Normalize("src/../lib/./utils", out);
  WriteString(out); WriteLn;              (* lib/utils *)

  Split("/home/user/hello.mod", dir, base);
  WriteString(dir); WriteLn;              (* /home/user *)
  WriteString(base); WriteLn;             (* hello.mod *)

  Extension("hello.mod", ext);
  WriteString(ext); WriteLn;              (* .mod *)

  RelativeTo("/a/b", "/a/c/d", out);
  WriteString(out); WriteLn;              (* ../c/d *)

  IF Match("src/Hello.mod", "*.mod") THEN
    WriteString("Modula-2 source"); WriteLn
  END
END
```
