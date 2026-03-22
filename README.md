# mx — Modula-2 Compiler

mx compiles Modula-2 to native executables via two backends: a **C backend** (transpile to C, then invoke the system C compiler) and an **LLVM backend** (emit LLVM IR, compile with clang). It implements **PIM4** with optional **Modula-2+** extensions (exceptions, reference types, objects, concurrency) via `--m2plus`.

## Why Modula-2?

- Grammar fits on a page (~40 reserved words)
- Separate interface (`.def`) and implementation (`.mod`) per module
- No implicit conversions, no header file resolution order issues
- Strict static type checking

## Why mx?

- **Two backends** — C backend for portability and inspectable output; LLVM backend for native DWARF debug info, LLVM-native exception handling, and RTTI.
- **Source-level debugging** — C backend uses `#line` directives; LLVM backend emits full DWARF metadata. Both support breakpoints and stepping in LLDB/GDB.
- **C FFI** — bind to any C library with `DEFINITION MODULE FOR "C"`.
- **Cross-compilation** — C backend: set `--cc` to a cross compiler. LLVM backend: set `--target`.
- **m2dap** — a Modula-2 Debug Adapter Protocol server for IDE debugging with M2-idiomatic variable display.

The toolchain also includes a package manager (`mxpkg`), an LSP server, a VS Code extension, and 33 libraries (see `libs/`).

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

## Tooling

`docs/ai/` contains structured references for use with coding agents:

- Language rules and compiler constraints
- Syntax patterns and idiomatic templates
- Module resolution and import mechanics
- API signatures for all 33 libraries
- Build system and project manifest format

See `docs/ai/CLAUDE.md` for reading order.

## Documentation

Language reference, library APIs, LSP configuration, and contributor guides are in [`docs/`](docs/README.md). Version history in [RELEASE_NOTES.md](RELEASE_NOTES.md).

## Project Layout

```
src/           Compiler (Rust) — C and LLVM backends
libs/          33 libraries (Modula-2)
tools/m2dap/   Debug adapter server (Modula-2+)
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
python3 tests/adversarial/run_adversarial.py --mode ci  # adversarial tests (C)
python3 tests/adversarial/run_adversarial.py --backend all  # adversarial tests (C + LLVM)
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
