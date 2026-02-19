# m2c LSP — Best-Effort Invariants

The m2c language server uses a **best-effort, eventually consistent** indexing
model. This document specifies what is guaranteed and what may be stale.

## Non-Negotiable Invariants

### 1. Open Documents: Always Fresh

All interactive features for open documents (those tracked by
`textDocument/didOpen`) operate on the **in-memory text** from `DocumentStore`:

- **Diagnostics** — published from in-memory analysis (debounced on
  `didChange`, immediate on `didSave` and `didOpen`).
- **Hover / Completion / Goto-def / Rename / References /
  DocumentHighlight / SignatureHelp / SemanticTokens / CodeActions /
  CallHierarchy** — always analyze the current `(uri, version)` pair.
  The analysis cache is keyed by `(doc_version, lock_hash)` and
  invalidated on every `didChange`.

The server **never** reads disk for an open document's content.

### 2. Workspace Index: Eventually Consistent

The `WorkspaceIndex` may be stale for non-open files:

- Files are indexed on initial startup, on `didSave`, and on lockfile/manifest
  changes.
- Files are stamped with `(mtime, size, content_hash)` — a change to any
  component triggers re-indexing.
- Open documents are indexed from in-memory text when opened and when
  diagnostics are published.

Between saves, the workspace index for a closed, modified-on-disk file may
reflect its prior state. This is acceptable: the index converges after the next
`didSave` or manual reindex.

### 3. Explicit Reindex: Forced Correctness

Users can force the index to reflect the current state:

- **Saving `m2.toml` or `m2.lock`** — evicts the project context, clears the
  analysis cache for that root, and triggers a full reindex.
- **`m2/reindexWorkspace` request** — clears all file stamps and rebuilds the
  entire index from scratch. For open documents, uses in-memory text.

After a forced reindex completes, the workspace index reflects disk state for
closed files and in-memory state for open files.

### 4. Index Never Blocks Interactive Requests

Hover, completion, rename, goto-def, and other interactive requests operate on
the per-document analysis cache, not the workspace index. They complete in
bounded time regardless of workspace size.

Cross-file references use the identity-based inverted index
(`refs_by_identity`), which is O(1) in lookup — not O(N files). If the index is
stale or unavailable, the server returns partial (same-file only) results rather
than blocking.

### 5. Cancellation

The server handles `$/cancelRequest`:

- Canceled request IDs are tracked in a `HashSet<i64>`.
- At handler entry, the server checks if the request was canceled. If so, it
  responds with JSON-RPC error code `-32800` (RequestCancelled) and does **not**
  mutate indexes or caches.
- After handling, the cancel ID is retired from the set.

## Timer-Based Debounced Diagnostics

The server uses a **threaded event loop** with three event sources:

- **Stdin reader thread** — reads JSON-RPC messages from stdin, sends
  `ServerEvent::Message(Json)` to the main channel.
- **Timer thread** — sends `ServerEvent::Tick` every N ms (default 50ms,
  configurable via `M2C_LSP_TICK_MS` env var).
- **StdinClosed** — sent when stdin reaches EOF.

On `textDocument/didChange`, the server:

1. Updates `DocumentStore` immediately (interactive requests see the new text).
2. Invalidates the analysis cache for that URI.
3. Records a pending-diagnostics timestamp.
4. Records a pending-index-update timestamp (workspace call graph).
5. Does **not** run full analysis immediately (unless debounce is disabled).

Pending diagnostics are flushed on `Tick` events when
`now - last_change > debounce_ms`. The default debounce is 250ms. Because the
timer fires independently of message arrival, diagnostics are published even
when no further user input arrives (fixing the previous inter-message
limitation).

Pending workspace index updates are flushed on `Tick` events when
`now - last_change > index_debounce_ms`. This ensures the workspace call graph
and symbol index reflect open-document edits without waiting for save.

Configuration:
- `initializationOptions.diagnostics.debounce_ms` — set in client config.
- `initializationOptions.diagnostics.index_debounce_ms` — set in client config.
- `M2C_LSP_DEBOUNCE_MS` environment variable — fallback for diagnostics.
- `M2C_LSP_INDEX_DEBOUNCE_MS` environment variable — fallback for index updates.
- `M2C_LSP_TICK_MS` — timer thread interval (default 50ms).
- Set debounce to `0` to disable (immediate diagnostics/indexing on every change).

`textDocument/didSave` always triggers immediate diagnostics (bypasses debounce).

## workDoneProgress

If the client advertises `window.workDoneProgress` capability, the server sends
progress notifications during:

- Initial workspace indexing (on `initialized`)
- Reindexing after manifest/lockfile changes
- Manual `m2/reindexWorkspace`

The server waits for the `window/workDoneProgress/create` response before
sending `begin`/`report`/`end` notifications (with a 500ms timeout). If the
client does not support progress, notifications are silently skipped. Progress
is always ended, even on cancellation.

## Multi-Root Workspace

The server supports multiple workspace folders from `initialize` params:

- `workspaceFolders` — each folder is checked for `m2.toml` and loaded as a
  project context.
- Fallback to `rootUri` if no workspace folders are provided.
- File URIs are routed to the best matching root by canonical path prefix.
- `workspace/symbol` searches across all roots (global limit 200).

## Manual Reindex

Send a request with method `m2/reindexWorkspace`:

```json
{"jsonrpc": "2.0", "id": 1, "method": "m2/reindexWorkspace", "params": {}}
```

Response:
```json
{"jsonrpc": "2.0", "id": 1, "result": {"files": 15, "symbols": 87}}
```

This clears all cached file stamps and rebuilds the index from scratch.

## Identity-Based Inverted Indexes

Cross-file references use **identity-based** inverted indexes for precision:

- `refs_by_identity: HashMap<IdentityKey, Vec<IdentityRef>>` — keyed by
  `"Module::Name::kind"` (e.g., `"Stack::Push::procedure"`).
- `defs_by_identity: HashMap<IdentityKey, IdentityLocation>` — definition sites.
- `symbols_by_name: HashMap<String, Vec<usize>>` — fallback for workspace/symbol
  search (case-insensitive substring match).

A `SymbolIdentity` is `(file, scope_id, module, name, kind)`:

- **Cross-file key** `"Module::Name::kind"` — used for cross-file rename and
  references. Two modules defining the same name produce different keys.
- **Local key** `"file::scope_id::name::kind"` — used for intra-module
  disambiguation. Two local procedures with the same name in different scopes
  get different local keys.

`find_refs_by_identity(key)` returns all refs for that identity — O(1) lookup.
`find_def_by_identity(key)` returns the definition location.

## Cross-File Rename

Cross-file rename uses identity-based indexes:

1. Resolve `SymbolIdentity` at cursor via `resolve_identity(symtab, name)`.
2. Collect same-file edits from `ReferenceIndex`.
3. Collect cross-file edits from `refs_by_identity`.
4. **Scope: workspace root only.** Files outside the workspace root (e.g., in
   dependency directories) are skipped with a log message. This prevents
   unintended edits to third-party code.
5. Build `WorkspaceEdit` with `changes` grouped by URI.

## Protocol Hygiene

- **Exit code**: 0 if `shutdown` was received before `exit`, 1 otherwise.
- **Post-shutdown rejection**: After `shutdown`, all requests except `exit` are
  rejected with JSON-RPC error code `-32600` (InvalidRequest).
- **Sync mode validation**: The server advertises `TextDocumentSyncKind::Full`
  (1). If a `didChange` event contains a `range` field (indicating incremental
  sync), a warning is logged to stderr. The server still processes the change
  using the full text.

## File Stamps

Files are stamped with:
- `mtime` (filesystem modification time)
- `size` (file size in bytes)
- `content_hash` (FNV-1a hash of file contents)

A file is re-indexed only when its stamp changes. This guards against false
negatives from mtime-only comparison (e.g., files touched without content
change, or mtime granularity issues).

## Semantic Tokens

`textDocument/semanticTokens/full` returns delta-encoded token data with the
standard LSP legend:

| Index | Token Type  |
|-------|------------|
| 0     | keyword    |
| 1     | type       |
| 2     | function   |
| 3     | variable   |
| 4     | parameter  |
| 5     | property   |
| 6     | namespace  |
| 7     | enumMember |
| 8     | number     |
| 9     | string     |

The server re-lexes the source (using `crate::lexer`), classifies each token via
`ReferenceIndex` + `SymbolTable`, and produces delta-encoded `[deltaLine,
deltaStartChar, length, tokenType, tokenModifiers]` tuples. Parameters are
distinguished from variables by checking parent procedure scope.

## Code Actions

`textDocument/codeAction` currently supports:

- **Missing import** — when an `undefined identifier 'Foo'` diagnostic is in
  range, searches `WorkspaceIndex` for a matching exported symbol and suggests
  `FROM Module IMPORT Foo;` as a quick fix (`WorkspaceEdit`).

Code actions only trigger on diagnostics that overlap the requested range.

## Call Hierarchy (Workspace-Wide)

The server supports the three-phase call hierarchy protocol with **workspace-wide
scope**:

1. **`textDocument/prepareCallHierarchy`** — returns a `CallHierarchyItem` for
   the procedure at cursor (must be `SymbolKind::Procedure`). Embeds a `data`
   field containing the procedure's `identityKey` and `moduleName` for
   workspace-wide resolution.
2. **`callHierarchy/incomingCalls`** — returns who calls the procedure **across
   all indexed files** in the workspace root. Uses `WorkspaceIndex.calls_in`.
3. **`callHierarchy/outgoingCalls`** — returns what the procedure calls across
   all indexed files. Uses `WorkspaceIndex.calls_out`.

### Workspace Call Graph

The workspace call graph is built during `WorkspaceIndex.rebuild_if_dirty()`:

- **Per-file call graph**: Each `AnalysisResult.call_graph` maps
  `caller_name → Vec<CallEdge>`. Built by walking the AST after semantic
  analysis.
- **Identity resolution**: Caller/callee names are resolved to identity keys
  using the file's symtab for unqualified calls and the AST designator module
  qualifier for qualified calls (`B.ProcB`).
  - **Top-level procedures**: `Module::Name::proc` (e.g., `Stack::Push::proc`).
  - **Nested procedures**: `Module::name@parent::proc` (e.g.,
    `M::helper@Outer1::proc`). The `@parent` suffix disambiguates same-named
    nested procedures in different scopes.
- **Workspace maps**:
  - `calls_out: HashMap<IdentityKey, Vec<WsCallEdge>>` — caller → callees.
  - `calls_in: HashMap<IdentityKey, Vec<WsCallEdge>>` — callee → callers.
  - `file_call_edges: HashMap<PathBuf, Vec<(caller_key, callee_key)>>` —
    per-file contribution tracking for incremental updates.
- **`fromRanges` accuracy**: Each `WsCallEdge` stores `site_col` and
  `site_end_col` (1-based, end exclusive) — the exact span of the callee
  identifier token at the call site. For qualified calls (`B.ProcB`), the span
  covers only the procedure name, not the module prefix.

### Best-Effort + Forced Correctness

- Call hierarchy is **best-effort** for closed files — converges after indexing.
- Open documents use in-memory analysis (invariant 1).
- **Debounced index updates**: `didChange` records a pending index update. The
  workspace call graph is rebuilt when `index_debounce_ms` expires, keeping the
  call graph fresh for open documents without blocking interactive requests.
- `m2/reindexWorkspace` rebuilds the call graph maps from scratch (forced
  correctness).
- `didSave` of `.mod`/`.def` files triggers re-indexing of that file, which
  updates the call graph.
- `didSave` of `m2.toml`/`m2.lock` triggers full reindex including call graph.

### Cancellation

The `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls` handlers
check for cancellation:

1. At handler entry (standard dispatch-level check).
2. After `rebuild_if_dirty()` completes (may take time on large workspaces).
3. Before root-scoping filter (if workspace root is set).

On cancellation, the handler responds with JSON-RPC error code `-32800` and
does not mutate the workspace index.

### Root Scoping

- Incoming/outgoing call results are filtered to the requesting file's workspace
  root. Callers/callees from other workspace roots are excluded.
- Dependency packages are included if they are part of the root's indexed file
  set (i.e., in the include paths).

### Fallback

- If the workspace index has no call graph data for a symbol (stale or not yet
  indexed), the handler falls back to single-file analysis from the open
  document's `AnalysisResult.call_graph`.
- Partial results are returned rather than blocking on reindex.

## Shared Project Resolver

Manifest (`m2.toml`) and lockfile (`m2.lock`) parsing lives in
`src/project_resolver.rs` — a crate-root module shared between the LSP and any
future driver integration. The LSP's `src/lsp/workspace.rs` re-exports all types
from the resolver.

The resolver provides:
- `Manifest::parse(content)` — section-aware key=value parsing.
- `Lockfile::parse(content)` — with `[dep.NAME]` section support.
- `Lockfile::content_hash(content)` — FNV-1a hash for cache keys.
- `find_project_root(path)` — walk-up directory search for `m2.toml`.
- `resolve_include_paths(root, manifest, lockfile, cli_paths)` — merge manifest
  includes, dep includes (from dep's own `m2.toml`), and CLI fallback paths.
- `ProjectContext::load(root, cli_paths)` — reads both files and resolves all
  paths.
