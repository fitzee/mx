# FAQ

## Why Modula-2+?

Modula-2 is a systems programming language designed by Niklaus Wirth. It has strong typing, modules, and clean syntax. Modula-2+ (from DEC SRC) adds exceptions, reference types, objects, and concurrency — features needed for real-world programs without abandoning M2's simplicity. m2c supports both standard PIM4 Modula-2 and these extensions via the `--m2plus` flag.

## Why transpile to C?

C is the universal portable assembly language. By emitting C, m2c inherits:

- Every platform's C compiler and optimizer
- Cross-compilation support (just set `--cc` to a cross compiler)
- Easy FFI with C libraries
- Debuggable output (the generated C is kept in `.m2c/gen/`)

## Why no async runtime?

The LSP server uses a synchronous event loop with two background threads (stdin reader + timer). This is deliberate:

- Zero external dependencies (no tokio, no async-std)
- Predictable, debuggable behavior
- Debounced analysis provides good responsiveness without async complexity
- The bottleneck is analysis, not I/O

## How do I report a bug?

File an issue at https://github.com/anthropics/claude-code/issues with:

- The `.mod` source that triggers the bug
- The compiler command you ran
- Expected vs. actual output
- `m2c --version-json` output

## Where is the registry/cache stored?

| Path | Contents |
|------|----------|
| `~/.m2pkg/registry/` | Package index and published packages |
| `~/.m2pkg/cache/` | Downloaded and cached packages |

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
M2C_SHOW_C_ERRORS=1 m2c program.mod -o program
```

## What Modula-2 standard does m2c follow?

PIM4 (Programming in Modula-2, 4th Edition by Niklaus Wirth). Keywords are always case-insensitive; identifiers are case-sensitive by default. Use `--case-insensitive` for full case insensitivity.

## Can I use m2c without m2pkg?

Yes. The compiler works standalone:

```bash
m2c myprogram.mod -o myprogram
```

m2pkg is optional and only needed for dependency management. The `m2c build`/`run`/`test` subcommands require an `m2.toml` manifest but do not require m2pkg.
