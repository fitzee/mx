# FAQ

## Why Modula-2+?

Modula-2 is a systems programming language designed by Niklaus Wirth. It has strong typing, modules, and clean syntax. Modula-2+ (from DEC SRC) adds exceptions, reference types, objects, and concurrency -- features needed for real-world programs without abandoning M2's simplicity. mx supports both standard PIM4 Modula-2 and these extensions via the `--m2plus` flag.

## Why two backends?

mx has a **C backend** (default) and an **LLVM backend** (`--llvm`).

Both backends are **CFG-driven** — all control flow (IF, WHILE, FOR, CASE, TRY/EXCEPT, etc.) is lowered to a control flow graph before code generation. Backends iterate basic blocks and emit terminators (goto/branch/switch/return). No structured control flow is reconstructed from HIR.

The C backend emits goto-based C with `#line` directives, inheriting every platform's C compiler and optimizer, cross-compilation (just set `--cc`), and easy FFI. The LLVM backend emits LLVM IR basic blocks, compiled by clang. It provides native DWARF debug info (variables visible in lldb with M2 type names) and RTTI for TYPECASE/REF/OBJECT. Use it when you need full IDE debugging with m2dap.

## Why no async runtime?

The LSP server uses a synchronous event loop with two background threads (stdin reader + timer). This is deliberate:

- Zero external dependencies (no tokio, no async-std)
- Predictable, debuggable behavior
- Debounced analysis provides good responsiveness without async complexity
- The bottleneck is analysis, not I/O

## How do I report a bug?

File an issue on the project's GitHub repository with:

- The `.mod` source that triggers the bug
- The compiler command you ran
- Expected vs. actual output
- `mx --version-json` output

## Where is the registry/cache stored?

| Path | Contents |
|------|----------|
| `~/.mxpkg/registry/` | Package index and published packages |
| `~/.mxpkg/cache/` | Downloaded and cached packages |

## How do I force the LSP to reindex?

In VS Code: Command Palette > "Modula-2+: Reindex Workspace"

Or send a JSON-RPC request to the LSP:
```json
{"jsonrpc": "2.0", "id": 1, "method": "m2/reindexWorkspace", "params": {}}
```

## Why don't cross-file references update immediately?

The LSP uses a **best-effort, eventually consistent** indexing model. The workspace index updates:

- **Immediately** when you save a file
- **After a debounce delay** (default 250ms) when you edit an open file
- **On demand** via the "Reindex Workspace" command

Between updates, cross-file features (references, rename, call hierarchy) may show stale results. Single-file features (hover, completion, diagnostics) always use current text. See [LSP invariants](lsp-invariants.md) for formal guarantees.

## How do I see C compiler errors?

By default, raw C compiler errors are hidden. To see them:

```bash
MX_SHOW_C_ERRORS=1 mx program.mod -o program
```

## What Modula-2 standard does mx follow?

PIM4 (Programming in Modula-2, 4th Edition by Niklaus Wirth). Keywords are always case-insensitive; identifiers are case-sensitive by default. Use `--case-insensitive` for full case insensitivity.

## How do I debug my Modula-2 program?

**In VS Code**: Run "Modula-2+: Create Debug Configuration" from the Command Palette, set breakpoints by clicking the gutter, and press `F5`. The generated config includes both m2dap (M2-native debugging) and CodeLLDB options. See [VS Code debugging](vscode.md#debugging) for details.

**From the command line** (C backend):

```bash
mx -g program.mod -o program
lldb ./program
(lldb) breakpoint set -f program.mod -l 10
(lldb) run
```

**From the command line** (LLVM backend — full variable inspection):

```bash
mx --llvm -g program.mod -o program
lldb ./program
(lldb) breakpoint set -f program.mod -l 10
(lldb) run
(lldb) frame variable -T
```

The LLVM backend emits native DWARF debug info, so `frame variable` shows local variables with their M2 names. For M2-idiomatic type display (`BOOLEAN` → `TRUE`/`FALSE`, `NIL` for null pointers), use m2dap in an IDE.

## What is m2dap?

m2dap is a Modula-2 Debug Adapter Protocol server. It wraps lldb as a subprocess and translates DAP messages (from VS Code, Zed, etc.) into lldb commands. It provides M2-idiomatic debugging: demangled procedure names (`Module.Proc`), M2 type names (`BOOLEAN`, `INTEGER`), and M2 value formatting (`TRUE`/`FALSE`, `NIL`, `CHR(N)`).

Build it with `cd tools/m2dap && mx build`. It requires the LLVM backend (`--llvm -g`) for full variable type information.

## Can I use mx without mxpkg?

Yes. The compiler works standalone:

```bash
mx myprogram.mod -o myprogram
```

mxpkg is optional and only needed for dependency management. The `mx build`/`run`/`test` subcommands require an `m2.toml` manifest but do not require mxpkg.
