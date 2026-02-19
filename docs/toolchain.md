# Using the toolchain

## Compiler

### Basic compilation

```bash
# Compile a module to an executable
m2c hello.mod -o hello

# Compile with Modula-2+ extensions (exceptions, REF types, objects)
m2c --m2plus program.mod -o program

# Emit C only (no compilation)
m2c --emit-c program.mod -o program.c

# Compile only (produce .o, no linking)
m2c -c module.mod
```

### Include paths

Use `-I` to add directories where the compiler searches for `.def` and `.mod` files:

```bash
m2c -I src -I vendor/lib/src main.mod -o main
```

The compiler searches include paths in order when resolving `FROM Module IMPORT ...` statements. The directory containing the input file is always searched first.

### Optimization

```bash
m2c -O0 program.mod -o debug_build    # no optimization (default)
m2c -O2 program.mod -o release_build  # optimized
```

### Linking

```bash
# Link with system libraries
m2c program.mod -lm -lpthread -o program

# Add library search paths
m2c program.mod -L/usr/local/lib -lmylib -o program

# Include extra C/object/archive files
m2c program.mod helper.c utils.o libstuff.a -o program
```

### C FFI

Foreign C modules use a special definition module syntax:

```modula2
DEFINITION MODULE FOR "C" CLib;
  PROCEDURE printf(fmt: ARRAY OF CHAR): INTEGER;
END CLib.
```

The compiler emits `extern` declarations with bare C names (no module prefix).

### Modula-2+ mode

Modula-2+ extends PIM4 with:

- **Exceptions**: `TRY`/`EXCEPT`/`FINALLY`, `RAISE`, `EXCEPTION` declarations
- **Reference types**: `REF T`, `REFANY`, `BRANDED REF`
- **Objects**: `OBJECT` types with vtable-based method dispatch
- **Concurrency**: `Thread`, `Mutex`, `Condition` standard modules; `LOCK` statement
- **Type dispatch**: `TYPECASE` on `REFANY` values
- **Module safety**: `SAFE`/`UNSAFE` annotations (parsed, not enforced)

Enable via CLI flag or manifest:

```bash
# CLI flag
m2c --m2plus program.mod -o program
```

```ini
# In m2.toml manifest
m2plus=true
```

### Diagnostics

```bash
# Standard error format (file:line:col: error: message)
m2c bad.mod
# => src/bad.mod:10:5: error: undefined identifier 'x'

# JSON diagnostics (JSONL to stderr)
m2c --diagnostics-json bad.mod
# => {"file":"src/bad.mod","line":10,"col":5,"severity":"error","kind":"semantic","message":"undefined identifier 'x'"}
```

### Case sensitivity

Keywords (`MODULE`, `BEGIN`, `IF`, etc.) are always case-insensitive. Identifiers are case-sensitive by default (PIM4 behavior). Use `--case-insensitive` for full case insensitivity:

```bash
m2c --case-insensitive program.mod -o program
```

### Feature gates

Conditional compilation via pragmas:

```modula2
(*$IF threading*)
FROM Thread IMPORT Fork, Join;
(*$ELSE*)
(* single-threaded fallback *)
(*$END*)
```

Enable features from the CLI:

```bash
m2c --feature threading --feature gc program.mod -o program
```

### Other flags

```bash
m2c --version-json          # machine-readable version info
m2c --print-targets         # supported target triples
m2c compile --plan build.json  # batch build from JSON plan
```

---

## Project build system

Projects with an `m2.toml` manifest can use the built-in build subcommands.

### Subcommands

```bash
m2c build [--release] [-v] [--cc <cmd>] [--feature <name>]...
m2c run [--release] [-v] [-- <args>...]
m2c test [-v] [--feature <name>]...
m2c clean
```

**build** compiles the project. With `--release`, uses `-O2`. Prints "up to date" if nothing changed.

**run** compiles and executes the binary. Arguments after `--` are passed to the program.

**test** compiles and runs the test entry point (default: `tests/Main.mod`).

**clean** removes the `.m2c/` build directory.

### Build artifacts

```
.m2c/
  build_state.json    # stamp cache (mtime, size, hash per file)
  bin/
    <name>            # compiled binary
  gen/
    <name>.c          # generated C (kept for debugging)
```

The build system stamps all source files and skips compilation if nothing changed (FNV-1a hash comparison).

---

## Package manager (m2pkg)

### Creating a project

```bash
m2pkg init
```

This creates an `m2.toml` manifest:

```ini
# m2.toml - package manifest
manifest_version=1
name=myproject
version=0.1.0
edition=pim4
entry=src/Main.mod
includes=src

[deps]

[cc]
# cflags=
# ldflags=
# libs=
# extra-c=
# frameworks=
```

### Manifest schema (`m2.toml`)

#### Core fields

| Key | Description | Default |
|-----|-------------|---------|
| `name` | Package name (required) | — |
| `version` | Semantic version X.Y.Z (required) | — |
| `entry` | Main module path | `src/Main.mod` |
| `m2plus` | Enable Modula-2+ extensions | `false` |
| `edition` | Set to `m2plus` to enable extensions | `pim4` |
| `includes` | Space-separated include directories | — |
| `manifest_version` | Manifest format version | `1` |

#### `[deps]` section

```ini
[deps]
mylib=path:../mylib         # local dependency
otherlib=0.2.0              # registry dependency
```

Each dependency must have its own `m2.toml`. The dependency's include directories are added to the compiler's search path.

#### `[cc]` section

C compiler integration (all values are space-separated):

| Key | Description | Example |
|-----|-------------|---------|
| `cflags` | Extra C compiler flags | `-Wall -Wextra` |
| `ldflags` | Linker flags | `-L/usr/local/lib` |
| `libs` | Libraries to link | `m pthread` |
| `extra-c` | Extra C source files | `libs/helper.c` |
| `frameworks` | macOS frameworks | `CoreFoundation IOKit` |

#### `[test]` section

| Key | Description | Default |
|-----|-------------|---------|
| `entry` | Test entry point | `tests/Main.mod` |
| `includes` | Test-only include directories | — |

#### `[features]` section

```ini
[features]
threading=false
gc=false
```

Features are enabled via `--feature` flags and control `(*$IF name*)` pragmas.

### Lockfile (`m2.lock`)

Generated by `m2pkg resolve`. Records resolved dependency versions, sources, and integrity hashes:

```ini
[package]
name=myproject
version=0.1.0

[dep.mylib]
version=0.1.0
source=local
sha256=abc123...
path=../mylib
```

Do not edit manually. Regenerate with `m2pkg resolve`.

### Registry and cache

| Path | Contents |
|------|----------|
| `~/.m2pkg/registry/` | Package index and published packages |
| `~/.m2pkg/cache/` | Downloaded package cache |

### Dependency resolution

1. Parse `m2.toml` manifest
2. For each dependency:
   - **Local** (`path:../lib`): resolve relative to project root
   - **Registry** (`0.2.0`): look up in `~/.m2pkg/registry/`, download if needed
3. Read each dependency's own `m2.toml` for transitive includes
4. Write resolved state to `m2.lock`
5. Compute include paths from all resolved dependencies

### Common workflows

**Create a new project:**
```bash
mkdir myproject && cd myproject
m2pkg init
mkdir -p src
cat > src/Main.mod << 'EOF'
MODULE Main;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello from m2pkg!");
  WriteLn;
END Main.
EOF
m2c run
```

**Add a local dependency:**
```bash
# Edit m2.toml:
# [deps]
# mylib=path:../mylib

m2pkg resolve          # regenerate m2.lock
m2c build              # rebuild with new dependency
```

**Build and run:**
```bash
m2c build              # compile
m2c run                # compile and execute
m2c run -- arg1 arg2   # pass arguments to program
m2c build --release    # optimized build
```

**Update dependencies:**
```bash
m2pkg resolve          # re-resolve all dependencies
m2c build              # rebuild
```

---

## Standard library modules

| Module | Description |
|--------|-------------|
| InOut | Character and string I/O (Read, Write, WriteString, WriteLn, ReadChar, WriteChar, etc.) |
| RealInOut | Real number I/O (ReadReal, WriteReal, WriteFixPt) |
| MathLib0 | Math functions (sqrt, sin, cos, exp, ln, arctan) |
| Strings | String operations (Assign, Concat, Length, Compare, Copy, Pos, Delete, Insert) |
| Terminal | Direct terminal I/O (Read, Write, WriteLn, WriteString) |
| FileSystem | File operations (Lookup, Close, ReadChar, WriteChar, ReadWord, WriteWord) |
| Storage | Heap allocation (ALLOCATE, DEALLOCATE) |
| SYSTEM | Low-level operations (WORD, BYTE, ADDRESS, ADR, TSIZE) |
| Conversions | Number/string conversions (IntToStr, StrToInt, CardToStr, StrToCard) |
| STextIO | ISO-style text I/O |
| SWholeIO | ISO-style whole number I/O |
| SRealIO | ISO-style real number I/O |
| Thread | Thread creation and joining (M2+ only) |
| Mutex | Mutual exclusion locks (M2+ only) |
| Condition | Condition variables (M2+ only) |
