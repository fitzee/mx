# m2c - Modula-2 to C Compiler

**v1.0.1** — A Modula-2 compiler that transpiles to readable C. Targets PIM4 with practical extensions (exceptions, objects, reference types, concurrency via `--m2plus`). Ships with a package manager, LSP server, VS Code extension, and libraries covering networking, graphics, crypto, database, compression, and async I/O.

79% compatibility with the GCC gm2 PIM4 test suite (383/483 tests pass). 150 unit tests, 558 adversarial tests across 8 compiler configurations.

**Not [m2sf/m2c](https://github.com/m2sf/m2c).** That project is a C99 translator for the Modula-2 R10 bootstrap kernel subset. **Not [GCC gm2](https://gcc.gnu.org/onlinedocs/gm2/).** That is the GNU Modula-2 frontend shipping with GCC 13, targeting PIM2/PIM4/ISO and compiling to native code through GCC's backend. This project transpiles PIM4+extensions to C and includes a complete toolchain.

## Why Modula-2?

For decades we settled on massive frameworks because abstractions help people build things faster. That tradeoff made sense when humans were writing all the code. But AI is now writing the majority of the code in a lot of shops, and the calculus is different. Take a complex language with a sprawling framework ecosystem and hand it to an LLM. The search space explodes. The model pulls in the wrong version of an API, hallucinates a method that doesn't exist, or produces something that compiles and appears to work on the surface. But do you actually know what it's doing? Can you read every line and verify it? With 200 transitive dependencies and six layers of abstraction, probably not.

Modula-2 was designed for modularity and safety. The grammar fits on a page. Every module has a clean separation between interface (.def) and implementation (.mod). There are no implicit conversions, no header file tangles, no undefined behavior traps hiding in innocent-looking code. When an LLM generates a Modula-2 module, you can read the whole thing. The compiler is strict enough to reject the mistakes that slip through in languages with more surface area.

If this sounds familiar, it should. Clojure took the same road from a different direction. Rich Hickey's "simple made easy" mantra, the insistence on a tiny core, libraries over frameworks, explicit data flow over hidden magic. When the search space is small and the idioms are consistent, code is easier to reason about, whether the one reasoning is a person or a model. Modula-2 arrives at the same place through rigid module boundaries, VAR parameters that make mutation visible at every call site, and a strict type system that leaves nothing implicit.

### AI-assisted development

The m2c libraries were built using AI coding with a human in the loop for architectural decisions. The strict compiler caught bad output that looser languages would have silently accepted.

You don't need to know Modula-2 to work this way. If you can read Pascal or Ada, you can already read it. If you can't, it takes about an hour. The language has roughly 40 reserved words. No operator overloads, no template metaprogramming, no lifetime annotations, no borrow checker. A procedure does what its signature says. A module exports what its definition file lists. You describe what you want, the LLM writes the module, the compiler tells you if it's wrong.

## Install

Requires Rust (`cargo`) and a C compiler (`cc`/`clang`/`gcc`).

```bash
git clone <repo> m2c && cd m2c
make install
```

Add to your shell profile:
```bash
export PATH="$HOME/.m2c/bin:$PATH"
```

Optional dependencies: OpenSSL 3 (`brew install openssl@3`), SQLite3 (`brew install sqlite3`), zlib (`brew install zlib`).

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

### Project builds

```bash
m2c build     # compile
m2c run       # compile and run
m2c test      # run tests
```

### VS Code

```bash
code --install-extension tools/vscode-m2plus/m2plus-0.1.0.vsix
```

## What's new in 1.0.1

### Bug fixes

- **Enum variant scope pollution in multi-module codegen** — Enum variant names (`OK`, `Invalid`, `OutOfMemory`, etc.) shared across different modules no longer collide. Previously, `import_map` entries leaked between embedded modules, causing variant names to resolve to the wrong source module. Each embedded module now starts with a clean import scope, and bare-key `enum_variants` entries are only registered for the main module.
- **Open array high bound missing in cross-module calls** — When multiple imported modules exported procedures with the same name (e.g., `Init`), calling the FROM-imported one with an open array parameter could omit the `_high` argument in the generated C. The symtab's `lookup_any` found the wrong module's procedure first, returning param info without the open array flag. FROM-import prefixed lookup now takes priority over bare-name symtab lookup, matching the existing FuncCall path.
- **Cross-platform build support** — Homebrew-specific include/library paths (`/opt/homebrew`) are now gated behind `[cc.feature.MACOS]` in library m2.toml files. The build system auto-injects `MACOS` or `LINUX` as implicit platform features at build time. The compiler driver gates GC paths and `-framework` flags on `cfg!(target_os = "macos")`. Libraries now build on Linux with system-installed packages (e.g., `libssl-dev`, `liblmdb-dev`) without extra flags.

## What's new in 1.0.0

### Codegen improvements

- **POINTER TO RECORD** — Anonymous record types inside pointer declarations now generate correct C struct definitions. Self-referential pointer-to-record types (linked lists, trees) work correctly.
- **WITH on pointer-to-record** — `WITH p^ DO` resolves fields through the pointer's base record type.
- **Multi-name pointer fields** — `left, right: POINTER TO Foo` now emits separate C declarations so both names are pointers (not just the first).
- **SET OF inline enum** — `TYPE s = SET OF (a, b, c)` emits the enum constants and a uint32_t set type with MIN/MAX macros.
- **Char literals in set operations** — Single-character string literals in INCL, EXCL, IN, set constructors, and array indices are emitted as C char literals instead of string pointers.
- **Module-level variable forward references** — Procedures can reference module-level variables declared after them. Variables are emitted before procedure bodies.
- **Constant forward references** — Constants referencing later-declared constants are topologically sorted before emission.
- **Nested module procedure hoisting** — Procedures inside local modules within a procedure are hoisted to file scope (C doesn't allow nested function definitions).
- **Nested procedure name mangling** — Same-named procedures nested in different parents get unique C names (`Alpha_Helper`, `Beta_Helper`) to avoid collisions.
- **MIN/MAX macros** — User-defined enumeration, subrange, and set-of-enum types emit `m2_min_`/`m2_max_` macros for use with the MIN/MAX builtins.
- **File type mapping** — `File` is only mapped to `m2_File` when imported from FileSystem/FIO, not when it's a user-defined type.

### Test coverage

- 150 cargo unit tests
- 558 adversarial tests
- 79% gm2 PIM4 compatibility (383/483), up from 54% (260/483)

## Documentation

Documentation covers language support (PIM4 and Modula-2+ features), toolchain usage, the package manager, LSP configuration, and per-module API reference for all libraries. Available in `docs/` and through the VS Code extension's documentation panel.

## Project layout

```
src/           Compiler (Rust)
tools/m2pkg/   Package manager (Modula-2+)
tools/vscode-m2plus/  VS Code extension
libs/          Libraries
tests/         Unit, integration, adversarial, conformance
docs/          Documentation
sdk/           Platform SDKs
```

## Tests

```bash
cargo test                                            # 150 unit tests
bash tests/run_all.sh                                 # integration tests
bash tests/conformance.sh                             # conformance tests
python3 tests/adversarial/run_adversarial.py --mode ci  # 558 adversarial tests (ASan+UBSan, 8 compiler configs)
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
