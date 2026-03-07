# m2c - Modula-2 to C Compiler

A Modula-2 compiler that transpiles to readable C. Targets PIM4 with practical extensions (exceptions, objects, reference types, concurrency via `--m2plus`). Ships with a package manager, LSP server, VS Code extension, and libraries covering networking, graphics, crypto, database, compression, and async I/O.

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
cargo test                                    # unit tests
bash tests/run_all.sh                         # integration tests
bash tests/conformance.sh                     # conformance tests
python3 tests/adversarial/run_adversarial.py  # adversarial tests (ASan+UBSan)
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
