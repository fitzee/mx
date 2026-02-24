# Language Support

What m2c supports across standard Modula-2 (PIM4) and the Modula-2+ extensions.

## PIM4 Modula-2

### Module System

| Feature | Status |
|---------|--------|
| Program modules (`MODULE`) | Supported |
| Definition modules (`.def`) | Supported |
| Implementation modules (`.mod`) | Supported |
| `FROM Module IMPORT name` | Supported |
| `IMPORT Module` (qualified access) | Supported |
| `FROM Module IMPORT Name AS Alias` | Supported (m2c extension) |
| `EXPORT QUALIFIED` | Supported |
| Opaque types in `.def` | Supported |
| Separate compilation | Supported (via C translation units) |

### Types

| Feature | Status |
|---------|--------|
| `INTEGER`, `CARDINAL` | Supported (32-bit) |
| `LONGINT`, `LONGCARD` | Supported (64-bit) |
| `REAL`, `LONGREAL` | Supported (float, double) |
| `COMPLEX`, `LONGCOMPLEX` | Supported (ISO) |
| `BOOLEAN`, `CHAR` | Supported |
| `BITSET` | Supported |
| `WORD`, `BYTE`, `ADDRESS` | Supported (SYSTEM types) |
| `PROC` | Supported (procedure type) |
| Enumeration types | Supported |
| Subrange types | Supported |
| `ARRAY [lo..hi] OF T` | Supported |
| Open array parameters (`ARRAY OF T`) | Supported (with `HIGH`) |
| `RECORD ... END` | Supported |
| Variant records (`CASE tag OF`) | Supported |
| `SET OF` | Supported |
| `POINTER TO T` | Supported |
| Procedure types | Supported |
| Type aliases | Supported |

### Statements

| Feature | Status |
|---------|--------|
| Assignment (`:=`) | Supported |
| `IF / ELSIF / ELSE / END` | Supported |
| `WHILE / DO / END` | Supported |
| `REPEAT / UNTIL` | Supported |
| `FOR / TO / BY / DO / END` | Supported |
| `LOOP / EXIT` | Supported |
| `CASE / OF / ELSE / END` | Supported |
| `WITH / DO / END` | Supported |
| `RETURN` | Supported |
| Procedure calls | Supported |

### Declarations

| Feature | Status |
|---------|--------|
| Constants (`CONST`) | Supported (including constant expressions) |
| Variables (`VAR`) | Supported |
| Types (`TYPE`) | Supported |
| Procedures and functions | Supported |
| Nested procedures | Supported |
| `VAR` parameters (pass by reference) | Supported |
| Value parameters | Supported |
| Forward declarations | Not supported |

### Built-in Procedures and Functions

| Category | Builtins |
|----------|----------|
| Arithmetic | `ABS`, `ODD`, `MAX`, `MIN` |
| Type conversion | `ORD`, `CHR`, `VAL`, `FLOAT`, `LFLOAT`, `TRUNC`, `LONG`, `SHORT` |
| Character | `CAP` |
| Array | `HIGH` |
| Size | `SIZE`, `TSIZE` |
| Set operations | `INCL`, `EXCL` |
| Increment/decrement | `INC`, `DEC` |
| Memory | `NEW`, `DISPOSE`, `ADR` |
| Bitwise | `SHL`, `SHR`, `BAND`, `BOR`, `BXOR`, `BNOT`, `SHIFT`, `ROTATE` |
| Complex numbers | `CMPLX`, `RE`, `IM` (ISO) |
| Control | `HALT` |
| Coroutines | `NEWPROCESS`, `TRANSFER`, `IOTRANSFER` (stub — not implemented at runtime) |

### Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`, `-`, `*`, `/`, `DIV`, `MOD` |
| Comparison | `=`, `#` (`<>`), `<`, `>`, `<=`, `>=` |
| Logical | `AND`, `OR`, `NOT` |
| Set | `+` (union), `-` (difference), `*` (intersection), `/` (symmetric diff), `IN` |
| Pointer | `^` (dereference) |
| Range | `..` |

### Standard Library Modules

| Module | Description |
|--------|-------------|
| `InOut` | Console I/O: `ReadInt`, `WriteInt`, `WriteString`, `WriteLn`, etc. |
| `RealInOut` | `ReadReal`, `WriteReal`, `WriteFloat` |
| `MathLib0` / `MathLib` | `sqrt`, `sin`, `cos`, `exp`, `ln`, `arctan`, `entier` |
| `Strings` | `Length`, `Assign`, `Concat`, `Compare`, `Pos`, `Copy`, `Delete`, `Insert` |
| `Terminal` | `Read`, `Write`, `WriteString`, `WriteLn` |
| `Storage` | `ALLOCATE`, `DEALLOCATE` |
| `SYSTEM` | `WORD`, `BYTE`, `ADDRESS`, `ADR`, `TSIZE` |
| `Files` | File I/O operations |
| `Args` | Command-line argument access |

### C Interop

| Feature | Status |
|---------|--------|
| Foreign definitions (`DEFINITION MODULE FOR "C"`) | Supported |
| `EXPORTC` pragma (`(*$EXPORTC "name"*)`) | Supported |
| Extra `.c` / `.o` / `.a` files | Supported (via driver flags or `[cc]` manifest) |
| `-l` / `-L` linker flags | Supported |

### Conditional Compilation

| Feature | Status |
|---------|--------|
| `(*$IF feature*)` / `(*$ELSE*)` / `(*$END*)` | Supported |
| `--feature <name>` CLI flag | Supported |
| `[features]` in `m2.toml` | Supported |

---

## Modula-2+ Extensions (`--m2plus`)

Enabled with `--m2plus` on the command line, or `edition=m2plus` in `m2.toml`.

### Exception Handling

```modula2
EXCEPTION MyError;

PROCEDURE Risky();
BEGIN
  RAISE MyError
END Risky;

BEGIN
  TRY
    Risky();
  EXCEPT MyError DO
    WriteString("caught MyError"); WriteLn;
  EXCEPT
    WriteString("catch-all"); WriteLn;
  FINALLY
    WriteString("always runs"); WriteLn;
  END;
END Example.
```

| Feature | Status |
|---------|--------|
| `EXCEPTION` declarations | Supported |
| `RAISE` | Supported |
| `TRY / EXCEPT / FINALLY / END` | Supported |
| Named exception handlers (`EXCEPT Name DO`) | Supported |
| Catch-all handler (`EXCEPT stmts`) | Supported |
| `FINALLY` block | Supported (runs on both normal and exception paths) |
| Exception propagation | Supported (unmatched exceptions re-raise) |

Implementation: setjmp/longjmp frame stack with `M2_TRY`/`M2_CATCH`/`M2_ENDTRY` C macros. No heap allocation for exception frames.

### Reference Types

```modula2
TYPE IntRef = REF INTEGER;

VAR r: IntRef;
BEGIN
  r := NEW(IntRef);
  r^ := 42;
END
```

| Feature | Status |
|---------|--------|
| `REF T` (typed references) | Supported |
| `REFANY` (untyped reference) | Supported |
| `BRANDED REF T` | Supported |
| `NEW` / `DISPOSE` for REF types | Supported |
| Dereference with `^` | Supported |

Implementation: `malloc`/`free` by default. Optional Boehm GC with `-DM2_USE_GC`.

### Object-Oriented Programming

```modula2
TYPE Shape = OBJECT
  x, y: INTEGER;
METHODS
  PROCEDURE Draw();
  PROCEDURE Area(): REAL;
END;

TYPE Circle = Shape OBJECT
  radius: REAL;
OVERRIDES
  PROCEDURE Draw();
  PROCEDURE Area(): REAL;
END;
```

| Feature | Status |
|---------|--------|
| `OBJECT` types | Supported |
| Fields | Supported |
| `METHODS` (virtual methods) | Supported |
| `OVERRIDES` | Supported |
| Single inheritance | Supported |

Implementation: vtable-based dispatch.

### Concurrency

```modula2
FROM Thread IMPORT Fork, Join, ThreadHandle;
FROM Mutex IMPORT Create, Lock, Unlock, MutexHandle;

VAR mu: MutexHandle;
BEGIN
  Create(mu);
  LOCK mu DO
    (* critical section *)
  END;
END
```

| Feature | Status |
|---------|--------|
| `Thread` module (`Fork`, `Join`) | Supported |
| `Mutex` module (`Create`, `Lock`, `Unlock`) | Supported |
| `Condition` module (condition variables) | Supported |
| `LOCK mu DO ... END` statement | Supported |

Implementation: pthreads. `M2_USE_THREADS` define emitted only when concurrency modules are imported.

### Type Dispatch

```modula2
VAR r: REFANY;
BEGIN
  TYPECASE r OF
  | IntRef(i):  WriteInt(i^, 0);
  | RealRef(f): WriteReal(f^, 0);
  ELSE
    WriteString("unknown type");
  END;
END
```

| Feature | Status |
|---------|--------|
| `TYPECASE` | Supported |
| `SAFE` / `UNSAFE` module annotations | Parsed (not enforced) |

---

## Tooling

### Project Build System

```bash
m2c init myproject       # scaffold m2.toml + src/Main.mod
m2c build                # compile project
m2c build -g             # compile with debug info
m2c run                  # compile and run
m2c test                 # compile and run tests
m2c clean                # remove build artifacts
```

Manifest-driven builds via `m2.toml` with `[deps]`, `[cc]`, `[features]`, and `[test]` sections.

### Package Manager (m2pkg)

Self-hosted package manager written in Modula-2+.

```bash
m2pkg init               # create m2.toml
m2pkg build              # build with dependencies
m2pkg run                # build and run
m2pkg publish            # publish to registry
m2pkg fetch              # download dependencies
m2pkg resolve            # resolve dependency versions
m2pkg lock               # generate lockfile
m2pkg verify             # verify lockfile integrity
```

- Local dependencies: `mylib=path:../mylib`
- Registry dependencies: `mylib=0.1.0` (semver with `^`, `~`, `>=` ranges)
- Transitive dependency resolution
- SHA-256 integrity verification
- JWT-authenticated publishing over HTTP/2

### LSP Server

Built into the compiler, activated with `m2c --lsp`.

| Feature | Status |
|---------|--------|
| Diagnostics (errors/warnings) | Supported |
| Hover (type info + docs) | Supported |
| Go to definition | Supported |
| Code completion (scope-aware) | Supported |
| Document symbols | Supported |
| Workspace symbols | Supported |
| Rename | Supported |
| Semantic tokens | Supported |
| Call hierarchy (incoming/outgoing) | Supported (workspace-wide) |
| Signature help | Supported |
| Document highlight | Supported |
| Code actions | Supported |

### VS Code Extension

- Syntax highlighting (TextMate grammar)
- Language configuration (brackets, comments, folding)
- Integrated documentation browser
- Debug configuration generator (CodeLLDB)
- Task provider (`m2c build/run/test/clean`)

### Debug Support

```bash
m2c build -g             # build with DWARF debug info
lldb .m2c/bin/myproject  # debug with source-level stepping in .mod files
```

Emits `#line` directives mapping C output back to `.mod` source lines. macOS: generates `.dSYM` bundles via `dsymutil`.

---

## Libraries

m2c ships with libraries for networking, async I/O, graphics, and more. See [library documentation](README.md#libraries) for details.

| Library | Description |
|---------|-------------|
| m2futures | Promises/futures for single-threaded async |
| m2evloop | Event loop with I/O watchers and timers |
| m2stream | Transport-agnostic byte streams (TCP, TLS) |
| m2sockets | POSIX/BSD socket networking |
| m2tls | TLS via OpenSSL/LibreSSL |
| m2http | HTTP client (HTTP/1.1 + HTTP/2) |
| m2http2 | HTTP/2 framing + HPACK |
| m2http2server | HTTP/2 server with routing and middleware |
| m2rpc | Length-prefixed RPC framing |
| m2auth | JWT HS256, Ed25519 PASETO, policy engine |
| m2gfx | SDL2-based 2D graphics |
| m2log | Structured logging |
| m2bytes | Byte buffers and binary codecs |
| m2alloc | Arena and pool allocators |
| m2fsm | Table-driven finite state machine |
| m2cli | CLI argument parser |
| m2sys | C shim (file I/O, exec, SHA-256, tar) |
