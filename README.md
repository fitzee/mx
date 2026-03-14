# mx — Modula-2 Compiler

A Modula-2 compiler that transpiles to readable C, then invokes the system C compiler for native executables.

mx implements **PIM4** (Programming in Modula-2, 4th Edition) with optional **Modula-2+** extensions (exceptions, reference types, objects, concurrency) enabled via `--m2plus`. The toolchain includes 33 libraries, a package manager, an LSP server, and a VS Code extension.

It is aimed at engineers building tooling, services, or systems software who want a small language, strict compilation, straightforward C interoperability, and a codebase that AI coding agents can reason about reliably.

## Why Modula-2?

Modula-2 was designed for modularity and safety. The grammar fits on a page. Every module has a clean separation between interface (`.def`) and implementation (`.mod`). There are no implicit conversions, no header file tangles, and the type system catches errors that looser languages accept silently. The language is small enough that both humans and LLMs produce auditable output — when an AI generates a module, you can read the whole thing.

## Why mx?

mx transpiles to C rather than emitting native code directly. This means:

- **Portable** — any platform with a C compiler is a target. Cross-compile by setting `--cc`.
- **Debuggable** — `#line` directives let you set breakpoints and step through `.mod` source in LLDB/GDB.
- **FFI-friendly** — C interop is trivial. Bind to any C library with `DEFINITION MODULE FOR "C"`.
- **Readable output** — the generated C is human-readable, so you can inspect exactly what the compiler produces.

The toolchain also includes project builds (`mx build/run/test`), a self-hosted package manager (`mxpkg`), an LSP server, and 33 libraries covering networking, HTTP/2, TLS, async I/O, graphics, databases, and authentication.

## Install

Requires Rust (`cargo`) and a C compiler (`cc`/`clang`/`gcc`).

```bash
git clone https://github.com/fitzee/mx.git && cd mx
make install
```

Add to your shell profile:
```bash
export PATH="$HOME/.mx/bin:$PATH"
```

OpenSSL 3 is required (`brew install openssl@3` on macOS, `sudo apt install libssl-dev` on Linux). Optional: SQLite3, zlib.

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

mx hello.mod -o hello && ./hello
```

### Project builds

```bash
mx build     # compile
mx run       # compile and run
mx test      # run tests
```

### VS Code

```bash
code --install-extension tools/vscode-m2plus/m2plus-*.vsix
```

## AI-Assisted Development

The mx libraries were built using AI coding agents with human oversight for architecture. The compiler's strict type checking and explicit module interfaces catch incorrect generated code early.

You don't need to know Modula-2 to work this way. If you can read Pascal or Ada, you can already read it. The language has roughly 40 reserved words — no operator overloads, no template metaprogramming, no lifetime annotations. A procedure does what its signature says. A module exports what its definition file lists.

To set up an AI coding agent for mx, point it at `docs/ai/`. The files there provide:

- **Language rules** — hard constraints the compiler enforces
- **Syntax cheatsheet** — copy-paste patterns for every construct
- **Idiomatic patterns** — templates for common tasks
- **Module resolution** — how imports work, what libraries exist
- **API reference** — procedure signatures for all 33 libraries
- **Build system** — project manifests, dependencies, debug builds

See `docs/ai/CLAUDE.md` for the recommended reading order.

## Documentation

Full documentation is in [`docs/`](docs/README.md) — language reference, toolchain usage, library API docs, LSP configuration, VS Code integration, and contributor guides. See [release notes](RELEASE_NOTES.md) for version history.

## Project Layout

```
src/           Compiler (Rust)
libs/          33 libraries (Modula-2)
tools/mxpkg/   Package manager (Modula-2+)
tools/vscode-m2plus/  VS Code extension
examples/      Categorized examples and demos
tests/         Unit, adversarial, conformance
docs/          Documentation
```

## Tests

```bash
cargo test                                              # unit tests
bash tests/run_all.sh                                   # integration tests
bash tests/conformance.sh                               # conformance tests
python3 tests/adversarial/run_adversarial.py --mode ci  # adversarial tests
```

## License

MIT License

Copyright (c) 2026 Matt Fitzgerald

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
