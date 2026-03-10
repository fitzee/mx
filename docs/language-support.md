# Language Support

What mx supports across standard Modula-2 (PIM4) and the Modula-2+ extensions.

## Compatibility

mx is a **superset of PIM4 Modula-2**. Existing PIM4 code should compile with mx as-is. However, code using mx-specific extensions (import aliases, C FFI pragmas, conditional compilation, bitwise builtins, or Modula-2+ features) will not compile with other Modula-2 compilers such as GNU Modula-2 (gm2).

| Direction | Compatibility |
|-----------|--------------|
| Legacy M2 code -> mx | Should work (PIM4 compatible) |
| mx code -> other compilers | Only if no mx extensions are used |

## PIM4 Modula-2

### Module System

| Feature | Status |
|---------|--------|
| Program modules (`MODULE`) | Supported |
| Definition modules (`.def`) | Supported |
| Implementation modules (`.mod`) | Supported |
| `FROM Module IMPORT name` | Supported |
| `IMPORT Module` (qualified access) | Supported |
| `FROM Module IMPORT Name AS Alias` | Supported (mx extension) |
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
| Coroutines (`NEWPROCESS`, `TRANSFER`, `IOTRANSFER`) | Not supported (compiles but exits at runtime) |

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
| Coroutines | `NEWPROCESS`, `TRANSFER`, `IOTRANSFER` (declared for compatibility; not implemented -- exits with error at runtime) |

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

Implementation: `M2_ref_alloc` prepends an `M2_RefHeader` (type descriptor pointer) before each payload. `malloc`/`free` by default; optional Boehm GC with `-DM2_USE_GC` (falls back to malloc if `gc/gc.h` is unavailable).

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

Implementation: vtable-based dispatch. Each OBJECT type gets an `M2_TypeDesc` linked to its parent, enabling subtype-aware TYPECASE matching.

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
    IntRef (i):  WriteInt(i^, 0)
  | RealRef:     WriteString("a real")
  ELSE
    WriteString("unknown type")
  END;
END
```

| Feature | Status |
|---------|--------|
| `TYPECASE` | Supported |
| Subtype matching (OBJECT inheritance) | Supported |
| Variable binding `Type (var):` | Supported |
| `SAFE` / `UNSAFE` module annotations | Parsed (not enforced) |

Implementation: `M2_ISA` walks `M2_TypeDesc` parent chain with depth early-out. NIL falls through to ELSE.

---

## See also

- [Using the toolchain](toolchain.md) — compiler flags, project builds, debugging, environment variables
- [mxpkg package manager](mxpkg.md) — dependency management, manifests, registry
- [LSP capabilities](lsp.md) — diagnostics, hover, completion, rename, call hierarchy
- [VS Code integration](vscode.md) — extension setup, debugging, tasks
- [Library index](README.md#libraries) — networking, graphics, async, crypto, and more
