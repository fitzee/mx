# URI

URI parser for HTTP client use. Decomposes URI strings into scheme, host, port, path, query, and fragment components.

## Overview

`URI` is a zero-allocation URI parser that breaks a URI string into its constituent parts using fixed-size arrays with explicit length tracking. It is designed for HTTP client use and supports the standard URI form: `scheme://host[:port][/path][?query][#fragment]`.

## Design Goals

- **Zero heap allocation**: All components stored in fixed-size arrays within `URIRec`.
- **Explicit lengths**: Every component has an accompanying length field; no reliance on NUL-termination for internal operations.
- **Default port inference**: Automatically assigns port 80 for `http` and 443 for `https`.
- **Percent decoding**: Separate `PercentDecode` procedure for decoding `%XX` sequences.

## Architecture

```
Input: "http://example.com:8080/api/v1?key=val#section"

  ┌──────────┬─────────────┬──────┬────────┬─────────┬─────────┐
  │ scheme   │ host        │ port │ path   │ query   │ fragment│
  │ "http"   │"example.com"│ 8080 │"/api/v1│"key=val"│"section"│
  │ len=4    │ len=11      │      │ len=7  │ len=7   │ len=7   │
  └──────────┴─────────────┴──────┴────────┴─────────┴─────────┘
```

## Internal Data Structures

```modula2
TYPE
  URIRec = RECORD
    scheme   : ARRAY [0..MaxScheme-1] OF CHAR;    (* 16 bytes *)
    host     : ARRAY [0..MaxHost-1] OF CHAR;      (* 256 bytes *)
    port     : INTEGER;
    path     : ARRAY [0..MaxPath-1] OF CHAR;      (* 2048 bytes *)
    query    : ARRAY [0..MaxQuery-1] OF CHAR;     (* 2048 bytes *)
    fragment : ARRAY [0..MaxFragment-1] OF CHAR;  (* 256 bytes *)
    schemeLen   : INTEGER;
    hostLen     : INTEGER;
    pathLen     : INTEGER;
    queryLen    : INTEGER;
    fragmentLen : INTEGER;
  END;
```

Total size of `URIRec`: ~4640 bytes. Stack-allocated by the caller.

## Memory Model

All storage is within the `URIRec` record itself — no heap allocation. The caller declares a `URIRec` variable (typically on the stack) and passes it by reference to `Parse`.

## Error Model

| Status      | Meaning                                        |
|-------------|------------------------------------------------|
| `OK`        | Parse succeeded.                               |
| `Invalid`   | Empty input string.                            |
| `TooLong`   | A component exceeds its maximum array size.    |
| `BadScheme` | Missing or malformed scheme (no `://`).        |
| `BadHost`   | Empty host component.                          |
| `BadPort`   | Non-numeric or out-of-range port (0..65535).   |

## Performance Characteristics

- **Parse**: Single-pass O(n) scan, character-by-character.
- **PercentDecode**: O(n) scan with in-place hex conversion.
- **DefaultPort**: O(1) constant comparison.
- **RequestPath**: O(n) copy of path + query.

## Limitations

- No support for `userinfo@` in authority (RFC 3986 Section 3.2.1).
- No IP literal (IPv6) bracket parsing.
- Scheme is case-normalized to lowercase; host is not.
- Maximum component sizes are compile-time constants.
- No relative URI resolution.

## Future Extension Points

- IPv6 address literal support (`[::1]`).
- Userinfo parsing for authenticated URIs.
- URI normalization (case, path segment, percent-encoding normalization).
- Relative URI resolution against a base URI.

## API Reference

### Constants

| Constant      | Value | Description          |
|---------------|-------|----------------------|
| `MaxScheme`   | 16    | Scheme array size.   |
| `MaxHost`     | 256   | Host array size.     |
| `MaxPath`     | 2048  | Path array size.     |
| `MaxQuery`    | 2048  | Query array size.    |
| `MaxFragment` | 256   | Fragment array size. |

### Procedures

```modula2
PROCEDURE Parse(VAR s: ARRAY OF CHAR; VAR uri: URIRec): Status;
```

Parse a NUL-terminated URI string into components. Scheme is lowercased. Default port is assigned if none specified.

```modula2
PROCEDURE PercentDecode(VAR src: ARRAY OF CHAR; srcLen: INTEGER;
                        VAR dst: ARRAY OF CHAR;
                        VAR dstLen: INTEGER): Status;
```

Decode `%XX` hex sequences in `src[0..srcLen-1]` into `dst`. Non-encoded characters are copied verbatim.

```modula2
PROCEDURE DefaultPort(VAR scheme: ARRAY OF CHAR;
                      schemeLen: INTEGER): INTEGER;
```

Return the default port for a scheme: 80 for `http`, 443 for `https`, 0 for unknown.

```modula2
PROCEDURE RequestPath(VAR uri: URIRec;
                      VAR out: ARRAY OF CHAR;
                      VAR outLen: INTEGER): Status;
```

Build the HTTP request path from a parsed URI: `/path[?query]` or `/` if the path is empty.

## Example

```modula2
VAR uri: URIRec; st: URI.Status;
    url: ARRAY [0..255] OF CHAR;

url := "http://example.com/api/data?format=json";
st := Parse(url, uri);
(* uri.scheme = "http", uri.host = "example.com",
   uri.port = 80, uri.path = "/api/data",
   uri.query = "format=json" *)
```

## See Also

- [HTTPClient](HTTPClient.md) — Uses URI for request targeting
- [Net-Architecture](Net-Architecture.md) — Overall networking stack design
