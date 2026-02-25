# m2c -Modula-2 to C Compiler

A modern superset of Modula-2 that compiles to fast, portable C. Write safe, modular code with the simplicity of PIM4 Modula-2 -then opt into exceptions, objects, reference types, and concurrency when you need them. The output is plain C that any system compiler can optimize, giving you native performance on every platform clang or gcc supports.

**Why Modula-2 in 2026?** It's the original systems language designed for modularity and safety. m2c extends it with the features modern applications actually need -TRY/EXCEPT, heap-allocated reference types, vtable-based objects, pthreads concurrency -while keeping the language small and the compiled output readable. No runtime, no garbage collector (unless you want one), no hidden allocations.

## Highlights

- **Compiles to C** -readable output, native speed, runs anywhere clang/gcc does
- **Modula-2+ extensions** -exceptions, REF types, objects, concurrency, TYPECASE (`--m2plus`)
- **17 libraries** for networking, crypto, graphics, async I/O, and more
- **Self-hosted package manager** (m2pkg) with registry, dependency resolution, and semver
- **Full LSP server** with VS Code extension -completions, hover, go-to-definition, call hierarchy
- **500+ tests** -unit, integration, adversarial, conformance, with sanitizer coverage

## What's in this repository

| Component | Location | Description |
|-----------|----------|-------------|
| **m2c** compiler | `src/` | Modula-2 to C compiler with built-in LSP server |
| **m2pkg** package manager | `tools/m2pkg/` | Self-hosted package manager (written in Modula-2+) |
| **VS Code extension** | `tools/vscode-m2plus/` | Language support via m2c's LSP server |
| **Libraries** | `libs/` | 17 packages for networking, graphics, crypto, and more |
| **Standard library** | `src/stdlib.rs` | InOut, RealInOut, MathLib, Strings, Terminal, etc. |
| **macOS SDK** | `sdk/macos/v1/` | ObjC++ runtime, shaders, M2 library modules |

## Quickstart

### Build from source

```bash
cargo build --release
sudo cp target/release/m2c /usr/local/bin/m2c
```

### Hello world

```bash
cat > hello.mod << 'EOF'
MODULE Hello;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello, world!");
  WriteLn;
END Hello.
EOF

m2c hello.mod -o hello && ./hello
```

### Modula-2+ extensions

```bash
m2c --m2plus myprogram.mod -o myprogram
```

### Project builds (m2.toml manifest)

```bash
m2c build              # compile
m2c run                # compile and run
m2c test               # run tests
m2c clean              # remove artifacts
```

### Package manager

```bash
m2pkg init             # create m2.toml
m2pkg resolve          # fetch and lock dependencies
m2pkg build            # compile with all deps
m2pkg run              # build and run
```

## Libraries

All in `libs/`, installable via m2pkg or usable as local path dependencies.

| Library | Description |
|---------|-------------|
| **m2gfx** | SDL2-based graphics -windows, drawing, events, fonts, textures, pixel buffers |
| **m2http** | HTTP client with DNS resolution and TLS |
| **m2http2** | HTTP/2 framing and HPACK compression |
| **m2http2server** | HTTP/2 server framework |
| **m2tls** | TLS 1.2/1.3 via OpenSSL |
| **m2sockets** | TCP/UDP socket abstraction |
| **m2stream** | Transport-agnostic byte streams |
| **m2futures** | Promises/futures, single-threaded async, pool-based allocation |
| **m2evloop** | Event loop with kqueue/epoll polling |
| **m2bytes** | Byte buffers, binary codec, hex encoding |
| **m2alloc** | Arena and pool allocators (caller-provided buffers, zero malloc) |
| **m2fsm** | Table-driven finite state machines, O(1) dispatch |
| **m2auth** | JWT HS256 signing/verification, keyring |
| **m2log** | Structured logging (no heap allocation in log path) |
| **m2rpc** | RPC framework |
| **m2cli** | CLI argument parser |
| **m2sys** | C shim -file I/O, exec, SHA-256, paths, tar, timestamps |

## VS Code

```bash
cd tools/vscode-m2plus
npm install && npm run compile
ln -s "$(pwd)" ~/.vscode/extensions/m2plus
```

Restart VS Code and open a `.mod` or `.def` file. If `m2c` is not on your PATH, set `m2c.serverPath` in settings. Reindex: Command Palette > "Modula-2+: Reindex Workspace".

## Tests

```bash
cargo test                                  # 141 unit tests
bash tests/run_all.sh                       # 76 integration tests
bash tests/conformance.sh                   # 22 conformance tests
python3 tests/adversarial/run_adversarial.py  # 296 adversarial tests (ASan+UBSan)
```

## Project layout

```
src/                    Compiler source (Rust)
  main.rs               CLI entry, subcommand routing
  lexer.rs              Tokenizer (case-insensitive keywords)
  parser.rs             Recursive-descent parser -> AST
  ast.rs                AST node types
  sema.rs               Semantic analysis, type checking, scopes
  codegen.rs            C code generation
  driver.rs             Compilation orchestration
  build.rs              Project build system (m2c build/run/test/clean)
  analyze.rs            Analysis-only path (LSP, no codegen)
  project_resolver.rs   Manifest/lockfile parsing
  stdlib.rs             Standard library definitions + C runtime
  lsp/                  LSP server (completions, hover, goto-def, call hierarchy, ...)

tools/
  m2pkg/                Self-hosted package manager (Modula-2+)
  m2pkg0/               Rust bootstrapper for m2pkg
  vscode-m2plus/        VS Code extension (TypeScript)

libs/                   17 libraries (see table above)
tests/                  Integration, conformance, adversarial tests
docs/                   Documentation
sdk/                    Platform SDKs
example_apps/           Example programs
```

## Documentation

- [Language support](docs/language-support.md) -PIM4 and Modula-2+ feature coverage
- [Using the toolchain](docs/toolchain.md) -compiler, package manager, workflows
- [LSP capabilities](docs/lsp.md) -features, configuration, troubleshooting
- [VS Code integration](docs/vscode.md) -extension setup, settings, commands
- [Architecture](docs/architecture.md) -compiler pipeline, LSP internals
- [m2pkg reference](docs/m2pkg.md) -package manager details
- [FAQ](docs/faq.md)
- [All docs](docs/README.md)

## License

MIT
