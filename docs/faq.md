# FAQ

## Why Modula-2+?

Modula-2 is a systems programming language designed by Niklaus Wirth. It has strong typing, modules, and clean syntax. Modula-2+ (from DEC SRC) adds exceptions, reference types, objects, and concurrency -- features needed for real-world programs without abandoning M2's simplicity. mx supports both standard PIM4 Modula-2 and these extensions via the `--m2plus` flag.

## Why transpile to C?

C is the universal portable assembly language. By emitting C, mx inherits:

- Every platform's C compiler and optimizer
- Cross-compilation support (just set `--cc` to a cross compiler)
- Easy FFI with C libraries
- Source-level debugging via `#line` directives (compile with `-g` to debug `.mod` files directly in LLDB/GDB)

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

**In VS Code**: Run "Modula-2+: Create Debug Configuration" from the Command Palette, set breakpoints by clicking the gutter, and press `F5`. You need the [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) extension. See [VS Code debugging](vscode.md#debugging) for details.

**From the command line**:

```bash
mx -g program.mod -o program
lldb ./program
(lldb) breakpoint set -f program.mod -l 10
(lldb) run
```

The `-g` flag emits `#line` directives so debuggers show your `.mod` source directly. Variables, stepping, and breakpoints all work at the Modula-2 level.

## Can I use mx without mxpkg?

Yes. The compiler works standalone:

```bash
mx myprogram.mod -o myprogram
```

mxpkg is optional and only needed for dependency management. The `mx build`/`run`/`test` subcommands require an `m2.toml` manifest but do not require mxpkg.
