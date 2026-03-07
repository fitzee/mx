# m2c - Modula-2 to C Compiler

A modern superset of Modula-2 that compiles to fast, portable C. The output is plain C that any system compiler can optimize, giving you native performance on every platform clang or gcc supports. No runtime, no garbage collector (unless you want one), no hidden allocations.

**This is not [m2sf/m2c](https://github.com/m2sf/m2c)** and it is not [GCC gm2](https://gcc.gnu.org/onlinedocs/gm2/). m2sf/m2c is a C99 translator for the Modula-2 R10 bootstrap kernel subset. gm2 is the GNU Modula-2 frontend that shipped with GCC 13, targeting PIM2/PIM4/ISO dialects and compiling to native code through GCC's backend. This project takes a different approach: it targets PIM4 with practical extensions (exceptions, objects, reference types, concurrency), transpiles to readable C, and ships as a complete toolchain with an LSP server, a self-hosted package manager, and 28 libraries for networking, graphics, crypto, and more.

## Why Modula-2 in the modern world?

For decades we settled on massive frameworks because abstractions help people build things faster. That tradeoff made sense when humans were writing all the code. But the ground has shifted. AI is now writing the majority of the code in a lot of shops, and the calculus is different. Take a complex language with a sprawling framework ecosystem and hand it to an LLM. The search space explodes. The model pulls in the wrong version of an API, hallucinates a method that doesn't exist, or produces something that compiles and appears to work on the surface. But do you actually know what it's doing? Can you read every line and verify it? With 200 transitive dependencies and six layers of abstraction, honestly, probably not.

Modula-2 was designed from the ground up for modularity and safety. The language is small. The grammar fits on a page. Every module has a clean separation between interface (.def) and implementation (.mod). There are no implicit conversions, no header file tangles, no undefined behavior traps hiding in innocent-looking code. When an LLM generates a Modula-2 module, you can read the whole thing. There is nowhere for bugs to hide behind framework magic. The compiler is strict enough to reject the mistakes that slip through in languages with more surface area. These properties made it a great systems language in the 1980s, and they make it a surprisingly effective one in the age of AI-generated code.

If this philosophy sounds familiar, it should. Clojure took the same road from a completely different direction. Rich Hickey's "simple made easy" mantra, the insistence on a tiny core, libraries over frameworks, explicit data flow over hidden magic. The Clojure community has been saying for years that a small language without a massive framework ecosystem isn't a weakness. When the search space is small and the idioms are consistent, code is easier to reason about, whether the one reasoning is a person or a model. Modula-2 arrives at the same destination through different means: where Clojure gives you simplicity through functional programming and immutable data, Modula-2 gives it to you through rigid module boundaries, VAR parameters that make mutation visible at every call site, and a strict type system that leaves nothing implicit. Two very different traditions, same bet on simplicity paying off.

m2c extends PIM4 Modula-2 with the features that real applications need: TRY/EXCEPT exception handling, heap-allocated reference types, vtable-based objects, and pthreads concurrency. You opt into these with `--m2plus` and the base language stays untouched.

The compiler has been hammered from every direction. Over 500 tests cover the pipeline (unit, integration, adversarial with ASan/UBSan, conformance), and real projects have pushed it well beyond toy programs. The toolchain has been used to build HTTP/2 servers with multiplexed streams on a single thread, native macOS applications with SDL2 graphics and ObjC bridging, terminal tools with paged file backends and undo/redo, JWT authentication, SQLite integration, content-addressed storage, CLI utilities that compile down to ~100KB static binaries, and multi-module codebases spanning thousands of lines across dozens of modules. The HTTP/2 throughput in particular is genuinely impressive for what amounts to generated C running on one thread.

### LLMs and Modula-2

Modula-2's small grammar and rigid module structure make it one of the best languages for AI-assisted development. The type system catches mistakes immediately, module boundaries prevent the kind of spaghetti that derails code generation in larger languages, and procedure signatures are completely unambiguous. The m2c compiler libraries themselves were built using AI coding with a human in the loop for architectural decisions. The strict compiler rejected bad output that looser languages would have silently accepted.

You don't need to know Modula-2 to use it this way. If you can read Pascal or Ada, you can already read Modula-2. If you can't, it takes about an hour. The entire language has roughly 40 reserved words. There are no operator overloads, no template metaprogramming, no implicit conversions, no lifetime annotations, no borrow checker rules to internalize. A procedure does what its signature says. A module exports what its definition file lists. That's it. The syntax looks unfamiliar for about fifteen minutes, and then it just gets out of the way. You describe what you want to the LLM, the LLM writes the module, the compiler tells you if it's wrong. The learning curve is the language's smallest feature.

If you want to build real software with an LLM as your copilot, a language with a small surface area and strong guardrails is exactly what you want. Modula-2 fits that role better than almost anything else available.

## Highlights

- **Compiles to C** - readable output, native speed, runs anywhere clang/gcc does
- **Modula-2+ extensions** - exceptions, REF types, objects, concurrency, TYPECASE (`--m2plus`)
- **28 libraries** for networking, crypto, graphics, async I/O, and more
- **Self-hosted package manager** (m2pkg) with registry, dependency resolution, and semver
- **Full LSP server** with VS Code extension - completions, hover, go-to-definition, call hierarchy
- **500+ tests** - unit, integration, adversarial, conformance, with sanitizer coverage

## What's in this repository

| Component | Location | Description |
|-----------|----------|-------------|
| **m2c** compiler | `src/` | Modula-2 to C compiler with built-in LSP server |
| **m2pkg** package manager | `tools/m2pkg/` | Self-hosted package manager (written in Modula-2+) |
| **VS Code extension** | `tools/vscode-m2plus/` | Language support via m2c's LSP server |
| **Libraries** | `libs/` | 28 packages for networking, graphics, crypto, and more |
| **Standard library** | `src/stdlib.rs` | InOut, RealInOut, MathLib, Strings, Terminal, etc. |
| **macOS SDK** | `sdk/macos/v1/` | ObjC++ runtime, shaders, M2 library modules |

## Quick Start

### Prerequisites

- Rust toolchain (`cargo`) - [rustup.rs](https://rustup.rs)
- C compiler (`cc` / `clang` / `gcc`)
- Optional: OpenSSL 3 (for m2http, m2tls, m2auth) - `brew install openssl@3`
- Optional: SQLite3 (for m2sqlite) - `brew install sqlite3`
- Optional: zlib (for m2zlib) - `brew install zlib`

### Install

```bash
git clone <repo> m2c && cd m2c
make install
```

Add to your shell profile (`~/.zshrc` or `~/.bashrc`):
```bash
export PATH="$HOME/.m2c/bin:$PATH"
```

To install to a custom location:
```bash
make install PREFIX=/opt/m2c
# then use: export M2C_HOME=/opt/m2c
```

### VS Code

```bash
code --install-extension tools/vscode-m2plus/m2plus-0.1.0.vsix
```

Set `m2c.serverPath` to `m2c` in VS Code settings if it's on your PATH.

### Verify

```bash
m2c --version
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
| **m2gfx** | SDL2-based graphics - windows, drawing, events, fonts, textures, pixel buffers |
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
| **m2json** | JSON parser and generator |
| **m2rpc** | RPC framework |
| **m2cli** | CLI argument parser |
| **m2sys** | C shim - file I/O, exec, SHA-256, paths, tar, timestamps |

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

libs/                   28 libraries (see table above)
tests/                  Integration, conformance, adversarial tests
docs/                   Documentation
sdk/                    Platform SDKs
example_apps/           Example programs
```

## Documentation

- [Language support](docs/language-support.md) - PIM4 and Modula-2+ feature coverage
- [Using the toolchain](docs/toolchain.md) - compiler, package manager, workflows
- [LSP capabilities](docs/lsp.md) - features, configuration, troubleshooting
- [VS Code integration](docs/vscode.md) - extension setup, settings, commands
- [Architecture](docs/architecture.md) - compiler pipeline, LSP internals
- [m2pkg reference](docs/m2pkg.md) - package manager details
- [FAQ](docs/faq.md)
- [All docs](docs/README.md)

## License

MIT
