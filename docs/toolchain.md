# Using the toolchain

## Compiler

### Basic compilation

```bash
# Compile a module to an executable (C backend, default)
mx hello.mod -o hello

# Compile with LLVM backend
mx --llvm hello.mod -o hello

# Compile with Modula-2+ extensions (exceptions, REF types, objects)
mx --m2plus program.mod -o program

# Emit C only (no compilation)
mx --emit-c program.mod -o program.c

# Emit LLVM IR only (no compilation)
mx --emit-llvm program.mod

# Compile only (produce .o, no linking)
mx -c module.mod

# Compile with AddressSanitizer + UndefinedBehaviorSanitizer
mx --sanitize hello.mod -o hello
mx --sanitize --llvm hello.mod -o hello
```

### Backend selection

mx has two compilation backends:

| Backend | Flag | Compiler | Debug info | Exception handling |
|---------|------|----------|-----------|-------------------|
| C (default) | _(none)_ | system `cc` | `#line` directives | setjmp/longjmp |
| LLVM | `--llvm` | `clang` | Native DWARF | LLVM-native (`invoke`/`landingpad`) + SjLj for ISO EXCEPT |

Set the backend in `m2.toml` for project builds:

```ini
backend=llvm
```

### Include paths

Use `-I` to add directories where the compiler searches for `.def` and `.mod` files:

```bash
mx -I src -I vendor/lib/src main.mod -o main
```

The compiler searches include paths in order when resolving `FROM Module IMPORT ...` statements. The directory containing the input file is always searched first.

### Optimization

```bash
mx -O0 program.mod -o debug_build    # no optimization (default)
mx -O2 program.mod -o release_build  # optimized
```

### Debug builds

```bash
# Compile with debug info
mx -g program.mod -o program

# Debug build via build subcommand
mx build -g
```

Debug mode enables source-level debugging of Modula-2 programs.

**C backend** (`-g`):

1. Emits C `#line` directives mapping generated C back to `.mod` source lines
2. Uses a two-step compile: `.c` -> `.o` (kept on disk) -> executable
3. Runs `dsymutil` on macOS to create a `.dSYM` debug symbol bundle
4. Sets stdout to unbuffered for immediate I/O when stepping in a debugger

The C compiler flags used in debug mode: `-g -O0 -fno-omit-frame-pointer -fno-inline -gno-column-info`

**LLVM backend** (`--llvm -g` or `--llvm --debug`):

1. Emits native DWARF metadata (`DICompileUnit`, `DISubprogram`, `DILocalVariable`, `DIGlobalVariable`)
2. Variables are visible in lldb with their M2 names and types
3. Full `#dbg_declare` records for local variables and parameters
4. Runs `dsymutil` on macOS for `.dSYM` bundles

```bash
# C backend debugging
mx -g hello.mod -o hello
lldb ./hello

# LLVM backend debugging (native DWARF, variable inspection)
mx --llvm -g hello.mod -o hello
lldb ./hello
(lldb) breakpoint set -f hello.mod -l 7
(lldb) run
(lldb) frame variable -T
```

For IDE debugging with M2-idiomatic variable display, see [m2dap](#m2dap-debug-adapter) and [VS Code integration -- Debugging](vscode.md#debugging).

### m2dap debug adapter

m2dap is a Modula-2 Debug Adapter Protocol (DAP) server that wraps lldb and provides M2-idiomatic debugging in IDEs. It translates DAP messages to lldb CLI commands and formats variables with M2 type names.

```
IDE (VS Code / Zed)
  ↕ DAP (JSON over stdio)
m2dap
  ↕ lldb CLI (bidirectional pipes)
lldb
  ↕ DWARF
M2 binary (mx --llvm -g)
```

Features:
- Breakpoints, stepping (over/into/out), continue, pause
- Stack traces with demangled M2 procedure names (`Module.Proc`)
- Variable inspection with DWARF type names (`BOOLEAN`, `INTEGER`, `CHAR`, etc.)
- M2 value formatting: `TRUE`/`FALSE`, character literals, `NIL` for null pointers
- Record and array display

Build m2dap:

```bash
cd tools/m2dap && mx build
```

See [VS Code integration](vscode.md#debugging) for IDE setup.

### Linking

```bash
# Link with system libraries
mx program.mod -lm -lpthread -o program

# Add library search paths
mx program.mod -L/usr/local/lib -lmylib -o program

# Include extra C/object/archive files
mx program.mod helper.c utils.o libstuff.a -o program
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

Enable Modula-2+ extensions (exceptions, REF types, objects, concurrency) via CLI flag or manifest:

```bash
mx --m2plus program.mod -o program
```

```ini
# In m2.toml manifest
edition=m2plus
```

See [language support](language-support.md#modula-2-extensions---m2plus) for the full feature matrix.

### Diagnostics

```bash
# Standard error format (file:line:col: error: message)
mx bad.mod
# => src/bad.mod:10:5: error: undefined identifier 'x'

# JSON diagnostics (JSONL to stderr)
mx --diagnostics-json bad.mod
# => {"file":"src/bad.mod","line":10,"col":5,"severity":"error","kind":"semantic","message":"undefined identifier 'x'"}
```

### Case sensitivity

Keywords (`MODULE`, `BEGIN`, `IF`, etc.) are always case-insensitive. Identifiers are case-sensitive by default (PIM4 behavior). Use `--case-insensitive` for full case insensitivity:

```bash
mx --case-insensitive program.mod -o program
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
mx --feature threading --feature gc program.mod -o program
```

### Cross-compilation

Use `--target` to select the target platform:

```bash
# Explicit target (both backends)
mx --target x86_64-linux program.mod -o program
mx --target aarch64-darwin --llvm program.mod -o program

# C backend: also set --cc to a cross compiler
mx --target aarch64-linux --cc aarch64-linux-gnu-gcc program.mod -o program-arm64
```

Supported target triples:

| Triple | Arch | OS | C ABI |
|--------|------|----|-------|
| `x86_64-linux` | x86_64 | Linux | System V |
| `aarch64-linux` | AArch64 | Linux | System V |
| `x86_64-darwin` | x86_64 | macOS | Darwin |
| `aarch64-darwin` | AArch64 | macOS | Darwin |

Short forms (`x86_64-linux`) and full forms (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`) are both accepted.

When `--target` is set:
- The LLVM backend emits the correct `target triple` and `target datalayout` in `.ll` output
- The driver selects target-appropriate linker flags (`-Wl,-dead_strip` on Darwin, `-Wl,--gc-sections` on Linux)
- The C backend emits `_Static_assert` guards that validate pointer size and type layout at C compile time
- Platform feature flags (`MACOS` / `LINUX`) are injected based on the target, not the host

Without `--target`, the host platform is used.

### Batch builds

The `compile --plan` command accepts a JSON file describing multiple compilation steps:

```bash
mx compile --plan build.json
```

See [build plan schema](mxpkg-build-plan.md) for the JSON format.

### Other flags

```bash
mx --version-json          # machine-readable version info (includes target_info)
mx --print-targets         # list supported target triples
mx --target <triple>       # set target platform (e.g. x86_64-linux, aarch64-darwin)
mx --sanitize              # enable ASan + UBSan (both backends)
```

---

## Project build system

Projects with an `m2.toml` manifest can use the built-in build subcommands.

### Subcommands

```bash
mx build [--release] [-g] [-v] [--cc <cmd>] [--target <triple>] [--sanitize] [--feature <name>]...
mx run [--release] [-g] [-v] [--sanitize] [-- <args>...]
mx test [-v] [--sanitize] [--feature <name>]...
mx clean
mx init [name]
```

**build** compiles the project. With `--release`, uses `-O2`. With `-g`/`--debug`, enables debug info and `#line` directives. Prints "up to date" if nothing changed.

**init** scaffolds a new project with `m2.toml`, `src/Main.mod`, and `tests/Main.mod`.

**run** compiles and executes the binary. Arguments after `--` are passed to the program.

**test** compiles and runs the test entry point (default: `tests/Main.mod`).

**clean** removes the `.mx/` build directory.

### Build artifacts

```
.mx/
  build_state.json    # stamp cache (mtime, size, hash per file)
  bin/
    <name>            # compiled binary
  gen/
    <name>.c          # generated C (kept for debugging)
```

The build system stamps all source files and skips compilation if nothing changed (FNV-1a hash comparison).

---

## Package manager (mxpkg)

The `mx build`/`run`/`test` subcommands handle single-project compilation. For dependency management, registry publishing, and multi-package workflows, use `mxpkg`. See the [mxpkg documentation](mxpkg.md) for commands, manifest schema, lockfile format, and dependency resolution.

---

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MX_HOME` | `~/.mx` | Install prefix for the toolchain |
| `MX_SHOW_C_ERRORS` | unset | Set to `1` to display raw C compiler errors |
| `MX` | auto-detected | Path to `mx` binary (used by mxpkg0 bootstrapper) |
| `MX_DOCS_PATH` | `$MX_HOME/docs` | Path to language documentation |
| `MX_LSP_DEBOUNCE_MS` | `250` | LSP diagnostics debounce delay (ms) |
| `MX_LSP_INDEX_DEBOUNCE_MS` | `250` | LSP workspace index update delay (ms) |
| `MX_LSP_TICK_MS` | `50` | LSP timer thread interval (ms) |
| `MXPKG_TOKEN` | unset | Registry authentication token |
| `MXPKG_INSECURE` | unset | Set to skip TLS certificate verification |

---

## Standard library modules

See [language support](language-support.md#standard-library-modules) for the full module reference. See the [library index](README.md#libraries) for extension libraries (networking, graphics, async, crypto, etc.).
