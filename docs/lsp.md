# LSP capabilities

The mx compiler includes a built-in LSP server, activated with `mx --lsp`. It communicates via JSON-RPC over stdio with no external dependencies.

## Starting the server

```bash
mx --lsp [--m2plus] [-I path]...
```

Typically launched by a client (VS Code, Neovim, etc.) rather than manually.

## Supported features

| Feature | Method | Description |
|---------|--------|-------------|
| Diagnostics | `textDocument/publishDiagnostics` | Real-time error/warning reporting (debounced) |
| Hover | `textDocument/hover` | Type signatures and symbol information |
| Go to definition | `textDocument/definition` | Same-file and cross-file navigation |
| Find references | `textDocument/references` | Identity-based cross-file reference search |
| Rename | `textDocument/rename` | Multi-file rename with prepare support |
| Completion | `textDocument/completion` | Scope-aware suggestions; triggers on `.` |
| Completion resolve | `completionItem/resolve` | Full type signatures for completion items |
| Signature help | `textDocument/signatureHelp` | Parameter hints; triggers on `(` and `,` |
| Document symbols | `textDocument/documentSymbol` | File outline (procedures, types, variables) |
| Workspace symbols | `workspace/symbol` | Cross-file symbol search (case-insensitive) |
| Document highlight | `textDocument/documentHighlight` | Highlight all uses of symbol under cursor |
| Semantic tokens | `textDocument/semanticTokens/full` | Rich syntax highlighting |
| Code actions | `textDocument/codeAction` | Quick fixes (e.g., add missing import) |
| Call hierarchy | `callHierarchy/*` | Incoming/outgoing calls (workspace-wide) |
| Cancellation | `$/cancelRequest` | Cancel in-flight requests |
| Reindex | `m2/reindexWorkspace` | Force full workspace reindex (custom method) |

## Indexing model

The server uses a **best-effort, eventually consistent** indexing model. See [lsp-invariants.md](lsp-invariants.md) for formal guarantees.

Key properties:

- **Open documents** are always analyzed from in-memory text (never stale).
- **Workspace index** may lag for closed files; converges after save or reindex.
- **Interactive requests** (hover, completion, etc.) never block on the workspace index.
- **Cross-file features** (references, rename, call hierarchy) use identity-based inverted indexes for O(1) lookup, with fallback to single-file results if the index is stale.

## Language documentation in hover

The LSP server provides structured language documentation for built-in types, procedures, keywords, and standard library modules. This documentation is served through:

- **Hover**: hovering over `INTEGER`, `NEW`, `MODULE`, `WHILE`, `InOut`, etc. shows a markdown panel with the signature, a one-line summary, and optional details.
- **Completion resolve**: built-in symbols include documentation in the completion detail panel.
- **Signature help**: built-in procedures (INC, DEC, etc.) include documentation in the signature help popup.

Documentation is centralized in `src/lsp/lang_docs.rs` and is never duplicated across handlers. User-defined symbols with real source locations always take precedence -- language docs only appear for builtins (symbols with no source file) and keywords.

Coverage includes:
- Built-in types: INTEGER, CARDINAL, REAL, LONGREAL, BOOLEAN, CHAR, BITSET, etc.
- Built-in procedures: NEW, DISPOSE, INC, DEC, ABS, ODD, CAP, ORD, CHR, VAL, HIGH, SIZE, etc.
- Built-in constants: TRUE, FALSE, NIL
- Standard library modules: InOut, RealInOut, MathLib0, Strings, Terminal, Storage, SYSTEM, etc.
- Keywords and constructs: MODULE, PROCEDURE, RECORD, ARRAY, IF, WHILE, FOR, CASE, etc.
- Modula-2+ extensions: TRY/EXCEPT/FINALLY, REF, REFANY, OBJECT, LOCK, TYPECASE, etc.

## Configuration

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MX_LSP_DEBOUNCE_MS` | `250` | Delay (ms) before publishing diagnostics after a change |
| `MX_LSP_INDEX_DEBOUNCE_MS` | `250` | Delay (ms) before updating workspace index after a change |
| `MX_LSP_TICK_MS` | `50` | Timer thread interval (ms) for debounce flush |

### initializationOptions

Clients can pass these in the `initialize` request:

```json
{
  "diagnostics": {
    "debounce_ms": 250,
    "index_debounce_ms": 250
  }
}
```

`initializationOptions` take precedence over environment variables.

Set debounce to `0` for immediate diagnostics/indexing on every keystroke (higher CPU usage).

### Project detection

The server auto-detects projects by walking up from the file's directory to find `m2.toml`. When found:

- Include paths are resolved from the manifest and lockfile
- `m2plus` mode is read from the manifest
- Saving `m2.toml` or `m2.lock` triggers automatic reindexing

### Multi-root workspaces

The server supports `workspaceFolders` from the initialize request. Each folder is checked for an `m2.toml` manifest. Cross-file features are scoped to the workspace root containing the file -- results from other roots are excluded.

If no workspace folders are provided, the server falls back to `rootUri`.

## Forced reindex

Send a custom request to force the workspace index to reflect current state:

```json
{"jsonrpc": "2.0", "id": 1, "method": "m2/reindexWorkspace", "params": {}}
```

Response:
```json
{"jsonrpc": "2.0", "id": 1, "result": {"files": 15, "symbols": 87}}
```

In VS Code: Command Palette > "Modula-2+: Reindex Workspace".

## Semantic token types

| Index | Token type |
|-------|-----------|
| 0 | keyword |
| 1 | type |
| 2 | function |
| 3 | variable |
| 4 | parameter |
| 5 | property |
| 6 | namespace |
| 7 | enumMember |
| 8 | number |
| 9 | string |

## Known limitations

### Analysis

- The LSP analyzes each file independently with transitive `.def` imports. There is no whole-program type checking.
- Analysis uses the pure lex/parse/sema path (`analyze.rs`); no C code is generated.

### Indexing

- The workspace index rebuilds entirely when any file changes (no incremental symbol updates). This is fast for typical project sizes but may be noticeable with hundreds of files.
- `workspace/symbol` returns at most 200 results.
- Name-based fallback for references cannot distinguish overloads or same-named procedures in different modules.

### Cross-file features

- Cross-file rename skips files outside the workspace root (dependency directories are read-only).
- If the workspace index is stale, cross-file features return partial (same-file only) results.

### Document sync

- The server uses full document sync (TextDocumentSyncKind 1). Incremental sync is not supported.

### Cancellation

- Cancellation is checked at handler entry and after index rebuilds, but not within individual handler logic. Long-running handlers cannot be interrupted mid-execution.

## Troubleshooting

### Server not starting

Verify `mx` is accessible:

```bash
mx --version-json
```

If not on PATH, set the full path in your editor's configuration.

### No diagnostics appearing

- Ensure the file has a `.mod` or `.def` extension.
- Check the editor's output panel for LSP errors (in VS Code: Output > "Modula-2+ Language Server").
- Try setting `MX_LSP_DEBOUNCE_MS=0` in your environment for immediate diagnostics.

### Stale cross-file references

Cross-file features use the workspace index, which updates on save. If results seem wrong:

1. Save all open files.
2. Run "Reindex Workspace" from the command palette.

### Missing definitions from dependencies

The server resolves dependencies from the `m2.lock` lockfile. If definitions from a dependency are not found:

1. Ensure `m2.lock` exists and is current: run `mxpkg resolve`.
2. Save `m2.lock` to trigger the LSP to reload project context.
3. Run "Reindex Workspace".

### Multi-root issues

Each workspace root is treated as a separate project. Cross-file features do not span roots. If you expect cross-root references, consolidate into a single root with appropriate include paths.

### Performance

- Default debounce (250ms) provides good responsiveness. Lower values increase CPU usage.
- Initial indexing of large workspaces may take a few seconds. The server sends `workDoneProgress` notifications if the client supports them.
- The tick timer (default 50ms) controls how frequently debounced items are flushed. Increasing `MX_LSP_TICK_MS` reduces CPU usage at the cost of latency.
