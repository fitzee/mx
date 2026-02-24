# m2c — Modula-2 to C Compiler

m2c is a Modula-2 compiler that transpiles PIM4 Modula-2 (with optional Modula-2+ extensions) to C, then invokes the system C compiler to produce native executables. Modula-2+ adds exceptions, reference types, object-oriented programming, and concurrency to standard Modula-2.

## What's in this repository

| Component | Location | Description |
|-----------|----------|-------------|
| **m2c** compiler | `src/` | Modula-2 to C compiler with built-in LSP server |
| **m2pkg** package manager | `tools/m2pkg/` | Self-hosted package manager (written in Modula-2) |
| **m2pkg0** bootstrapper | `tools/m2pkg0/` | Rust bootstrapper for building m2pkg |
| **VS Code extension** | `tools/vscode-m2plus/` | Language support via m2c's LSP server |
| **Standard library** | `src/stdlib.rs` | InOut, RealInOut, MathLib, Strings, Terminal, and more |
| **macOS SDK** | `sdk/macos/v1/` | ObjC++ runtime, shaders, M2 library modules |

## Quickstart

### Build from source

```bash
# Build the compiler
cargo build --release

# Install (requires sudo on macOS)
sudo cp target/release/m2c /usr/local/bin/m2c
```

### Compile and run a module

```bash
# Write a hello world
cat > hello.mod << 'EOF'
MODULE Hello;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello, world!");
  WriteLn;
END Hello.
EOF

# Compile and run
m2c hello.mod -o hello
./hello
```

### Compile with Modula-2+ extensions

```bash
m2c --m2plus myprogram.mod -o myprogram
```

### Project-based build (with m2.toml manifest)

```bash
# In a directory with m2.toml:
m2c build              # compile the project
m2c run                # compile and run
m2c test               # compile and run tests
m2c clean              # remove build artifacts
```

### Run tests

```bash
cargo test                        # 104 unit tests
bash tests/run_all.sh             # 72 integration tests (68 pass, 4 skipped)
bash tests/conformance.sh         # 22 conformance tests
```

## VS Code quickstart

```bash
# 1. Build the compiler (if not already done)
cargo build --release
sudo cp target/release/m2c /usr/local/bin/m2c

# 2. Build and install the extension
cd tools/vscode-m2plus
npm install
npm run compile
ln -s "$(pwd)" ~/.vscode/extensions/m2plus

# 3. Restart VS Code, open a .mod or .def file
```

If `m2c` is not on your PATH, set `m2c.serverPath` in VS Code settings to the full path.

To force the LSP to rebuild its index: Command Palette > "Modula-2+: Reindex Workspace".

## Project layout

```
src/                    Compiler source (Rust)
  main.rs               CLI entry point and subcommand routing
  lexer.rs              Tokenizer (keywords always case-insensitive)
  parser.rs             Recursive-descent parser → AST
  ast.rs                AST node types
  sema.rs               Semantic analysis (type checking, scope resolution)
  codegen.rs            C code generation
  driver.rs             Compilation orchestration (compile, link)
  build.rs              Project build system (m2c build/run/test/clean)
  analyze.rs            Analysis-only path for LSP (no C codegen)
  project_resolver.rs   Manifest/lockfile parsing, project detection
  stdlib.rs             Standard library module definitions + C runtime
  symtab.rs             Symbol table with scoped lookups
  types.rs              Type registry
  errors.rs             Error types and formatting
  json.rs               Minimal JSON parser (no dependencies)
  lsp/                  LSP server implementation
    server.rs           Event loop, request dispatch, debounce
    index.rs            Workspace index (symbols, refs, call graph)
    completion.rs       Scope-aware code completion
    hover.rs            Type information on hover
    goto_def.rs         Go-to-definition
    call_hierarchy.rs   Incoming/outgoing calls (workspace-wide)
    ...                 Other LSP handlers

tools/
  vscode-m2plus/        VS Code extension (TypeScript)
  m2pkg/                Self-hosted package manager (Modula-2)
  m2pkg0/               Rust bootstrapper for m2pkg

tests/                  Integration and conformance tests
libs/                   Support libraries
  m2sys/                C shim for m2pkg (file I/O, exec, SHA-256)
  m2cli/                Pure M2 CLI argument parser library
docs/                   Documentation
sdk/                    Platform SDKs
example_apps/           Example Modula-2 programs
```

## Documentation

- [Language support](docs/language-support.md) — what PIM4 and Modula-2+ features m2c supports
- [Documentation index](docs/README.md)
- [Using the toolchain](docs/toolchain.md) — compiler, package manager, workflows
- [LSP capabilities](docs/lsp.md) — features, configuration, troubleshooting
- [VS Code integration](docs/vscode.md) — extension setup, settings, commands
- [Architecture](docs/architecture.md) — compiler pipeline, LSP internals, contributing
- [FAQ](docs/faq.md) — common questions
- [LSP invariants](docs/lsp-invariants.md) — formal guarantees of the LSP server
- [m2pkg reference](docs/m2pkg.md) — package manager details
- [Build plan schema](docs/m2pkg-build-plan.md) — JSON build plan format

## License

MIT
