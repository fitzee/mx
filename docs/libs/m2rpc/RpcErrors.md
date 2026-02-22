# RpcErrors

Stable error codes and human-readable descriptions for the m2rpc framework. Provides a consistent error vocabulary shared between client and server, encoded in wire protocol error messages.

## Why Standard Error Codes?

RPC systems need a well-defined error taxonomy that both client and server understand. Unlike exceptions or arbitrary error strings, stable numeric codes enable programmatic error handling -- clients can test `errCode = Timeout` and implement retry logic, without parsing free-form text or relying on language-specific exceptions.

The m2rpc error codes occupy the range 1..99. Applications can define custom error codes starting at 100. Code 0 (Ok) is reserved as a sentinel for "no error" and never appears in wire protocol ERR messages.

## Constants

| Code | Name | Meaning |
|------|------|---------|
| 0 | `Ok` | No error (sentinel only, never sent in ERR messages) |
| 1 | `BadRequest` | Malformed frame or protocol violation |
| 2 | `UnknownMethod` | Server received a method name it doesn't recognize |
| 3 | `Timeout` | Client-side timeout -- no response before deadline |
| 4 | `Internal` | Server-side unrecoverable error (handler panic, etc.) |
| 5 | `TooLarge` | Frame exceeds MaxFrame limit |
| 6 | `Closed` | Transport closed or call canceled |

Application-defined error codes should start at 100 to avoid collisions with future framework extensions.

## Procedures

### ToString

```modula2
PROCEDURE ToString(code: CARDINAL; VAR s: ARRAY OF CHAR);
```

Convert an error code to a short human-readable string. The result is copied into the provided `s` array, truncated if necessary. For framework codes (1..6), returns the constant name (`"BadRequest"`, `"Timeout"`, etc.). Unknown codes return `"Unknown"`.

The returned strings are compile-time constants embedded in the module's read-only data section -- you do not need to deallocate them.

```modula2
VAR msg: ARRAY [0..31] OF CHAR;
ToString(Timeout, msg);
WriteString(msg);  (* "Timeout" *)
```

## Example

```modula2
MODULE ErrorDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM RpcErrors IMPORT Ok, BadRequest, UnknownMethod, Timeout,
                      Internal, TooLarge, Closed, ToString;

PROCEDURE ShowError(code: CARDINAL);
VAR msg: ARRAY [0..31] OF CHAR;
BEGIN
  WriteString("Error ");
  WriteCard(code, 0);
  WriteString(": ");
  ToString(code, msg);
  WriteString(msg);
  WriteLn
END ShowError;

BEGIN
  ShowError(Ok);              (* Error 0: Ok *)
  ShowError(BadRequest);      (* Error 1: BadRequest *)
  ShowError(UnknownMethod);   (* Error 2: UnknownMethod *)
  ShowError(Timeout);         (* Error 3: Timeout *)
  ShowError(Internal);        (* Error 4: Internal *)
  ShowError(TooLarge);        (* Error 5: TooLarge *)
  ShowError(Closed);          (* Error 6: Closed *)
  ShowError(100);             (* Error 100: Unknown *)
END ErrorDemo.
```
