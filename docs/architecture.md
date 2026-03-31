# Architecture

This document covers the internal architecture of mx for contributors.

## Compiler pipeline

```
Source (.mod/.def)
  -> TargetInfo (src/target.rs)   detect/parse target triple, populate layout info
  -> Lexer (src/lexer.rs)         tokenize into TokenKind stream
  -> Parser (src/parser.rs)       recursive-descent -> AST
  -> Sema (src/sema.rs)           type checking, scope resolution, symbol table
  -> HIR (src/hir_build/)         build_module: prebuilt HirModule with structural
                                  decls, lowered statements, procedure bodies
  -> CFG (src/cfg/)               build_cfg: control flow graph from HIR bodies
  -> CodeGen:
       C backend (src/codegen_c/)       CFG blocks -> C source (goto-based)
       LLVM backend (src/codegen_llvm/) CFG blocks -> LLVM IR basic blocks
  -> Driver (src/driver.rs)       invoke cc/clang, link
```

The compiler has two backends selected at compile time:

- **C backend** (default): emits goto-based C source with `#line` directives for debugging. Invokes the system C compiler.
- **LLVM backend** (`--llvm`): emits LLVM IR text (`.ll`), compiled by clang. Produces native DWARF debug info, LLVM-native exception handling, and RTTI for TYPECASE/REF/OBJECT types.

### Lexer

`Lexer::new(source, filename)` produces a `Vec<Token>`. Keywords are always matched case-insensitively (the input is uppercased before keyword lookup). Identifiers preserve original case by default; with `case_sensitive: false`, they are uppercased.

The lexer handles feature-gate pragmas (`(*$IF name*)` / `(*$ELSE*)` / `(*$END*)`) at the token level -- disabled blocks are skipped entirely, so the parser never sees them.

### Parser

`Parser::new(tokens)` implements a recursive-descent parser producing `CompilationUnit` (the top-level AST node). The AST types are defined in `src/ast.rs`.

The parser handles:
- Program modules (`MODULE ... END name.`)
- Definition modules (`DEFINITION MODULE ...`)
- Implementation modules (`IMPLEMENTATION MODULE ...`)
- Foreign C modules (`DEFINITION MODULE FOR "C" ...`)
- All PIM4 statements and expressions
- Modula-2+ extensions when enabled (TRY/EXCEPT, REF, OBJECT, etc.)

### Semantic analysis

`SemanticAnalyzer` (in `src/sema.rs`) is embedded inside each backend's `CodeGen` struct as `self.sema`. It performs:

- Symbol table construction with nested scope tracking
- Type checking and coercion
- Import resolution against registered definition modules
- Open array parameter tracking

The symbol table (`src/symtab.rs`) uses a scope stack (`Vec<usize>`) for nested scope management. Each scope has a parent, enabling walk-up lookup.

### HIR (High-level IR)

`src/hir.rs` defines a typed intermediate representation for designators, expressions, statements, and module structure. `src/hir_build/` provides `HirBuilder` for statement/expression lowering and `build_module()` for constructing a complete `HirModule` as a prebuilt compilation phase.

**Prebuilt HIR** (`build_module`): Runs after sema, before backend codegen. Constructs an `HirModule` containing:
- Structural declarations: type_decls, const_decls, global_decls, exception_decls, proc_decls
- Embedded modules: `HirEmbeddedModule` for each imported implementation module, with its own types/consts/globals/procs/init body
- Module body: init_body, except_handler, finally_handler (all pre-lowered to `Vec<HirStmt>`)
- Procedure declarations: `HirProcDecl` per procedure with full signature, lowered body, locals, nested procs, closure captures, and except handler. Bodies are populated at arbitrary nesting depth via recursive `build_nested_recursive`.

Both backends iterate structural declarations from `HirModule` for:
- C backend: record forward decls, type decls, const decls, exception decls, global var decls, proc forward decls, except/finally handlers
- LLVM backend: type decls, const decls, exception decls, global var decls, proc decls (via `gen_hir_proc_decl`), module body/except/finally

**TypeId → C name resolver**: A `typeid_c_names` map (populated from HirModule type_decls, def-module registration, and gen_type_decl emission) allows `type_id_to_c()` to resolve TypeIds to their C typedef names. Only non-structural types (records, enums, arrays, aliases) are registered to avoid cross-module conflicts with pointer/set types.

Key statement/expression lowering responsibilities:
- **Statement lowering**: Converts all AST statements (PIM4 and M2+) to `HirStmt`. WITH is desugared to field projections. FOR direction is pre-computed.
- **Expression lowering**: Converts AST expressions to `HirExpr` with resolved types. Constants are unwrapped to literals (except when projections are present). Single-char strings carry `TY_CHAR` with promotion to `TY_STRING` for open array contexts.
- **Designator resolution**: Maps AST designators to `Place` (base + projections), using sema's scope chain as the single source of truth for locality and type identity.
- **Call argument expansion**: Inserts `_high` companions for open array parameters, wraps VAR arguments as `AddrOf`. Skips `AddrOf` for args that are already pointers (open arrays, VAR params, fixed arrays).
- **WITH scope tracking**: Resolves bare field names to qualified record field accesses.
- **TYPECASE binding**: Registers branch variables with the correct REF type before lowering the body.
- **Scope-aware lookup**: Uses `current_scope` from sema, with `var_types_owned` checked before context-provided var_types (so dynamically registered variables like TYPECASE bindings are visible).

### Control Flow Graph (CFG)

`src/cfg/` constructs a verified control flow graph from HIR statement bodies. The CFG is the **single source of truth for control flow** in both backends — no structured IF/WHILE/CASE/LOOP constructs survive past this stage.

**Construction**: `build_cfg(body: &[HirStmt]) -> Cfg` lowers all HIR control flow statements into basic blocks with terminators. The driver calls this for every procedure body, module init body, embedded module init, and local module init after HIR construction (Phase 4b).

**Data model**:
- `BasicBlock`: sequential statements (`Vec<HirStmt>` — only Assign/ProcCall/Empty) + one `Terminator`
- `Terminator`: `Goto(BlockId)`, `Branch { cond, on_true, on_false }`, `Switch { expr, arms, default }`, `Return(Option<HirExpr>)`, `Raise(Option<HirExpr>)`
- `SwitchArm`: labels (`Vec<CaseLabel>`) + target block. `CaseLabel` is `Single(expr)`, `Range(lo, hi)`, or `Type(SymbolId, Option<binding_var>)`
- `handler: Option<BlockId>` per block — exception handler region annotation for TRY/EXCEPT

**Lowering**:
- IF/ELSIF/ELSE → Branch chains with merge block
- WHILE → Branch + Goto back-edge
- REPEAT → Goto body + Branch back-edge (condition reversed)
- FOR → init assign + cond Branch + body + latch (Add step) + Goto back
- LOOP/EXIT → Goto body + Goto back / Goto exit block
- CASE → Switch terminator with Single/Range labels
- TYPECASE → Switch terminator with Type labels + binding variable assignment
- TRY/EXCEPT/FINALLY → handler-annotated body blocks + catch dispatch (Branch chain) + finally blocks + Raise(None) for reraise
- LOCK → desugared to ProcCall(Lock) + body + ProcCall(Unlock)
- AND/OR → short-circuit Branch chains (real control flow, not eager evaluation)

**Proc-level EXCEPT**: The driver wraps procedure bodies that have `except_handler` in a synthetic `HirStmtKind::Try` before building the CFG, so exception dispatch is part of the CFG rather than a backend concern.

**Module-level FINALLY**: Folded into the module init body CFG via synthetic TRY wrapping. Both backends execute finally inline (no atexit).

**Invariants** (verified by `Cfg::verify()`):
- Entry block is always block 0
- All block IDs are sequential
- Every block has exactly one terminator
- All terminator targets are valid block IDs
- Handler targets are valid block IDs

**Post-construction passes**:
- `compute_preds()` — materializes predecessor lists from terminators
- `cleanup()` — `remove_unreachable()` (BFS from entry + handler roots) + `collapse_trivial_gotos()` (merge single-predecessor blocks)

### Code generation — C backend

`src/codegen_c/` emits C code from CFG and HIR, split across 10 modules:

| File | Purpose |
|------|---------|
| `mod.rs` | Core struct, state management, output buffer |
| `cfg_emit.rs` | CFG-driven body emission: block labels, terminators, SJLJ handler regions |
| `modules.rs` | Module-level codegen, imports, embedded impl modules |
| `decls.rs` | Procedure and variable declarations |
| `stmts.rs` | Statement dispatch (all statements route through HIR) |
| `hir_emit.rs` | HIR → C emission for statements (Assign/ProcCall only) and expressions |
| `exprs.rs` | Legacy AST expression helpers (escape functions, etc.) |
| `designators.rs` | HIR Place → C designator strings, name mangling |
| `types.rs` | TypeNode → C type strings, type name resolution |
| `m2plus.rs` | M2+ type/declaration codegen (REF, OBJECT, EXCEPTION) |

Key design decisions:

- **Module-prefixed names**: Imported symbols get `Module_Name` prefixes (e.g., `Stack_Push`).
- **Embedded implementations**: Imported module `.mod` files are inlined into the output C file, topologically sorted by dependencies.
- **Foreign modules**: `DEFINITION MODULE FOR "C"` modules emit `extern` declarations with bare C names.
- **EXPORTC pragma**: `(*$EXPORTC "c_name"*)` maps M2 procedure names to specific C names.
- **Record types**: Emitted as `struct` with `typedef struct X X;` forward declarations.
- **Open arrays**: Passed as pointer + high-bound pair.
- **VAR parameters**: Passed as pointers.
- **CFG-driven control flow**: All control flow (IF/WHILE/FOR/CASE/etc.) is emitted from CFG terminators as `goto L<n>;`, `if (cond) goto L<t>; else goto L<f>;`, and `return`. No structured C control flow constructs are generated.
- **Exception handling**: setjmp/longjmp via raw `m2_ExcFrame` fields. Handler regions are detected from CFG `block.handler` annotations. The backend sets up setjmp at handler region entry and pops the frame at region exit. Exception dispatch uses `frame.exception_id == M2_EXC_Name` comparisons.

### Code generation — LLVM backend

`src/codegen_llvm/` emits LLVM IR as text (`.ll` files). Key modules:

| File | Purpose |
|------|---------|
| `mod.rs` | Core struct, Val representation, semantic type queries, registration APIs |
| `cfg_emit.rs` | CFG-driven body emission: LLVM basic blocks, terminators, SJLJ handler regions |
| `modules.rs` | Module-level codegen: `gen_program_module`, `gen_implementation_module`, `gen_embedded_impl_module_by_name`, topo sort, import maps — all HIR-driven |
| `decls.rs` | HIR-based declaration emission: `gen_hir_type_decls_from`, `gen_hir_var_decls_global_from`, `gen_hir_const_decls_from`, `gen_hir_exception_decls_from`, `gen_hir_proc_decl` |
| `stmts.rs` | HIR statement emission (Assign/ProcCall only), builtin procedure expansion |
| `exprs.rs` | HIR expression emission, function calls, short-circuit AND/OR, COMPLEX builtins (CMPLX/RE/IM), NEW/DISPOSE |
| `designators.rs` | HIR Place → LLVM IR address/load, variant field offset computation, named array param handling |
| `types.rs` | TypeId resolution, LLVM type strings, float coercion, type_lowering delegation |
| `type_lowering.rs` | M2 types → LLVM IR types (variant record flattening, pseudo-field skip) |
| `llvm_types.rs` | LLVM type representation and IR emission |
| `stdlib_sigs.rs` | Standard library call signatures |
| `debug_info.rs` | DWARF metadata (DICompileUnit, DISubprogram, DILocalVariable) |

Key design decisions:

- **Val carries TypeId**: Every codegen result is `{name, ty, type_id}` where `type_id` tracks the semantic identity from sema, enabling correct aggregate handling and cross-module type disambiguation.
- **Aggregate invariant**: Records and arrays stay as addresses (pointers) in `gen_designator_load`. Callers that need values (return, struct-by-value) load explicitly. This prevents invalid SSA loads of aggregate types.
- **Sema-driven types**: All semantic questions (is this a pointer? what are the record fields?) are answered from the sema TypeRegistry, not LLVM type strings.
- **Zero AST dependencies**: All codegen reads from prebuilt HIR (`HirModule`, `HirProcDecl`, `HirEmbeddedModule`). The only AST import (`use crate::ast::*`) is for shared types (`BinaryOp`, `UnaryOp`, `ExprKind`) used by HIR expressions, plus `CompilationUnit` for entry-point dispatch.
- **Closure captures**: `HirProcDecl.closure_captures` is populated by hir_build with transitive propagation — grandchild captures are forwarded through parent env structs. The LLVM backend promotes captured variables to globals.
- **CFG-driven control flow**: All control flow is emitted from CFG terminators as `br label %B<n>`, `br i1 %cond, label %B<t>, label %B<f>`, and `ret`. Each CFG block maps to an LLVM basic block. No structured control flow reconstruction occurs.
- **Short-circuit evaluation**: AND/OR are lowered to Branch chains in the CFG (real control flow, not eager LLVM `and`/`or` instructions).
- **Floored DIV/MOD**: Calls `m2_div`/`m2_mod` runtime helpers for signed integer division (PIM4 requires floored, not truncated).
- **COMPLEX arithmetic**: Delegates to `m2_complex_add`/`sub`/`mul`/`div`/`neg`/`eq` runtime helpers. CMPLX/RE/IM builtins use LLVM `insertvalue`/`extractvalue`.
- **M2+ exception handling**: setjmp/longjmp-based `m2_ExcFrame` stack. The CFG annotates blocks with `handler: Option<BlockId>` for exception regions; the backend emits SJLJ frame setup/teardown inline based on handler transitions. TRY/EXCEPT/FINALLY and procedure-level EXCEPT are all lowered in the CFG; the backend handles only the SJLJ mechanism.
- **RTTI**: `M2_TypeDesc` globals for REF/OBJECT types, `M2_RefHeader` prepended to allocations, `M2_ISA` for TYPECASE runtime type checking.
- **DWARF debug info**: `DW_LANG_C99` (for lldb compatibility), `#dbg_declare` records (LLVM 19+ format), full DILocalVariable/DIGlobalVariable metadata.

### Target abstraction

`src/target.rs` defines `TargetInfo`, which formalizes platform semantics consumed by both backends, the driver, and the runtime. It is constructed once at the start of `driver::compile()` — either from the host platform or from an explicit `--target` triple — and passed by reference to both backends.

```rust
struct TargetInfo {
    triple: String,            // e.g. "x86_64-unknown-linux-gnu"
    arch: Arch,                // X86_64 | Aarch64
    os: Os,                    // Linux | Darwin
    pointer_bits: u32,         // 64 for all supported targets
    endian: Endianness,        // Little (all supported targets)
    c_abi: CAbi,               // SysV | Darwin
    int_layout: IntLayout,     // sizes of INTEGER, LONGINT, REAL, etc.
    alignments: AlignmentInfo, // struct + primitive alignment rules
    supports_setjmp: bool,     // true for all POSIX targets
}
```

Supported targets: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `aarch64-darwin`. All use LP64 data model.

Key APIs:
- `TargetInfo::from_host()` — detect from `std::env::consts::{ARCH, OS}`
- `TargetInfo::from_triple(s)` — parse short (`x86_64-linux`), full (`x86_64-unknown-linux-gnu`), or LLVM canonical (`arm64-apple-macosx14.0.0`) triples
- `llvm_triple()` / `llvm_datalayout()` — LLVM backend strings
- `type_size(tid, types)` / `type_align(tid, types)` — size/alignment for any TypeId (primitives, records, arrays, variants)
- `compute_record_layout(tid, types, target)` — field offsets with C ABI padding
- `emit_c_layout_guards(target)` — `_Static_assert` guards emitted into generated C for compile-time validation

The C backend emits layout guards after the runtime header to catch target mismatches at C compile time (e.g., cross-compiling with the wrong `--cc`). The LLVM backend uses `llvm_triple()` and `llvm_datalayout()` for the `.ll` header instead of detecting the host at codegen time.

The driver uses `target.is_darwin()` instead of `cfg!(target_os)` for linker flag selection, so `--target` correctly selects linker flags for the target platform rather than the host.

### Driver

`driver::compile()` orchestrates the full pipeline in five phases:

1. **Phase 1 — Parse `.def` files**: Starting from the main module's imports, transitively discover and parse all `.def` files into a `parsed_defs` map.
2. **Phase 2 — Topological sort and register `.def` files**: Topologically sort the parsed `.def` files by their import dependencies, then register each with sema in order. Pre-registers type names first (as opaques) so cross-module type references resolve during full registration.
3. **Phase 3 — Load `.mod` files**: For each implementation module, find and parse its `.mod` file. Recursively discover and register any `.def` files referenced by the `.mod` that weren't found in Phase 1 (ensures transitive dependencies like `URI.def` for `HTTPClient.mod` are registered in correct order before the module that needs them). Register implementation module types with sema.
4. **Phase 4 — Analyze, build HIR, build CFGs**: Run sema on the main compilation unit. Build prebuilt `HirModule` via `build_module()`. Then build CFGs for all bodies: procedure bodies (with proc-level EXCEPT folded into synthetic TRY), module init body (with EXCEPT/FINALLY folded in), embedded module inits, and local module inits. CFGs are stored on `HirProcDecl.cfg`, `HirModule.init_cfg`, and `HirEmbeddedModule.init_cfg`.
5. **Phase 5 — Backend emission**: Pass the `HirModule` (with pre-built CFGs) to the selected backend. Both backends iterate structural declarations from HIR for types/consts/globals/protos, then emit procedure and init bodies by walking CFG blocks — emitting labels, statements, and terminators. Write `.c` or `.ll`, invoke the system compiler, link.

The driver handles include path resolution, finding `.def`/`.mod` files, and constructing the compiler command line. The LLVM backend is selected with `--llvm` (full compilation) or `--emit-llvm` (emit `.ll` text only).

**Debug mode** (`-g`): The driver uses a two-step compile (`.c` -> `.o` -> executable) so the `.o` file stays on disk for DWARF debug info. On macOS, `dsymutil` creates a `.dSYM` bundle after linking. The codegen emits `#line` directives and `setvbuf(stdout, NULL, _IONBF, 0)` for unbuffered I/O. The C compiler receives `-g -O0 -fno-omit-frame-pointer -fno-inline -gno-column-info`.

**Release mode**: Single-step compile+link; the `.c` file is cleaned up after compilation.

## Analysis-only path (LSP)

`src/analyze.rs` provides `analyze_source()`, which runs lex -> parse -> sema without C codegen. This is the path used by the LSP server.

### AnalysisResult

```rust
pub struct AnalysisResult {
    pub ast: Option<CompilationUnit>,
    pub symtab: SymbolTable,
    pub types: TypeRegistry,
    pub scope_map: ScopeMap,
    pub ref_index: ReferenceIndex,
    pub call_graph: HashMap<String, Vec<CallEdge>>,
    pub diagnostics: Vec<CompileError>,
}
```

**ScopeMap** maps source positions (line, col) to scope IDs. Used by completion to show only in-scope symbols.

**ReferenceIndex** tracks all symbol references in a file (definition sites and usage sites). Used by rename, highlight, and goto-def.

**CallEdge** records call sites with callee name, optional module qualifier, and source position (including end column for exact token spans).

The `sema.into_results()` method extracts the symbol table, type registry, scope map, and errors from the semantic analyzer without requiring codegen.

## Symbol identity model

Cross-file features use identity-based keys to track symbols across the workspace.

### SymbolIdentity

Each symbol has:
- `file`: canonical path of the defining file
- `scope_id`: scope number (0 = module level)
- `module`: module name
- `name`: symbol name
- `kind`: procedure, type, variable, constant, or module

### Identity keys

- **Cross-file key**: `"Module::Name::kind"` (e.g., `"Stack::Push::proc"`). Used for cross-file rename and references.
- **Local key**: `"file::scope_id::Name::kind"`. Used for intra-module disambiguation.
- **Nested procedures**: `"Module::name@parent::proc"` (e.g., `"M::helper@Outer::proc"`). The `@parent` suffix disambiguates same-named nested procedures.

## WorkspaceIndex

`src/lsp/index.rs` maintains the workspace-wide index.

### Core data structures

| Structure | Type | Purpose |
|-----------|------|---------|
| `files` | `HashMap<PathBuf, IndexedFile>` | Per-file index data, keyed by canonical path |
| `symbols` | `Vec<WorkspaceSymbol>` | Flat symbol list for workspace/symbol search |
| `symbols_by_name` | `HashMap<String, Vec<usize>>` | Name -> symbol indices (case-insensitive) |
| `refs_by_identity` | `HashMap<IdentityKey, Vec<IdentityRef>>` | Cross-file references by identity |
| `defs_by_identity` | `HashMap<IdentityKey, IdentityLocation>` | Definition locations by identity |
| `refs_by_name` | `HashMap<String, Vec<IndexedRef>>` | Name-based fallback references |
| `calls_out` | `HashMap<IdentityKey, Vec<WsCallEdge>>` | Outgoing call edges per procedure |
| `calls_in` | `HashMap<IdentityKey, Vec<WsCallEdge>>` | Incoming call edges per procedure |
| `file_call_edges` | `HashMap<PathBuf, Vec<(key, key)>>` | Per-file call edge tracking for incremental updates |

### File stamps and invalidation

Files are stamped with `(mtime, size, content_hash)` using FNV-1a hashing. A file is re-indexed only when its stamp changes. Open documents are indexed from in-memory text, not disk.

### Rebuild cycle

`rebuild_if_dirty()` runs when the `dirty` flag is set:

1. Clear all inverted indexes
2. Iterate indexed files; extract symbols, refs, call edges
3. Build `symbols_by_name` index
4. Build `refs_by_identity` and `defs_by_identity`
5. Build `calls_out` and `calls_in` from extracted call edges
6. Clear `dirty` flag

## LSP server event loop

`src/lsp/server.rs` implements the main event loop.

### Threads

```
stdin_reader_thread  ->  ServerEvent::Message(Json)
timer_thread         ->  ServerEvent::Tick  (every MX_LSP_TICK_MS, default 50ms)
main loop:
  on Tick:    flush_pending_diagnostics()
              flush_pending_index_updates()
  on Message: dispatch request/notification
```

### Debounced updates

On `textDocument/didChange`:

1. Update `DocumentStore` immediately (interactive requests see new text)
2. Invalidate analysis cache for that URI
3. Record URI in `pending_diagnostics` with current timestamp
4. Record URI in `pending_index_updates` with current timestamp

On `Tick` events, entries older than the debounce threshold are flushed:
- Diagnostics: re-analyze and publish
- Index: re-index from in-memory text and rebuild workspace index

`textDocument/didSave` bypasses debounce (immediate analysis + index update).

### Cancellation

`$/cancelRequest` adds the request ID to a `HashSet<i64>`. At handler entry, cancelled requests get a `-32800` (RequestCancelled) error response. Call hierarchy handlers also check cancellation after index rebuilds.

### Request dispatch

A `match` on the JSON-RPC `method` string routes to handler functions. Unknown methods with an `id` get a `-32601` error. Unknown notifications are silently dropped.

## Project resolver

`src/project_resolver.rs` is a crate-root module shared between the LSP server and the build system.

It provides:
- `Manifest::parse(content)` -- INI-style parsing with `[section]` support
- `Lockfile::parse(content)` -- with `[dep.NAME]` sections
- `Lockfile::content_hash(content)` -- FNV-1a hash for cache keys
- `find_project_root(path)` -- walk-up directory search for `m2.toml`
- `ProjectContext::load(root, cli_paths)` -- reads manifest + lockfile, resolves all include paths

The LSP's `src/lsp/workspace.rs` re-exports these types.

## Build system

`src/build.rs` implements the `mx build`/`run`/`test`/`clean` subcommands.

### Stamp-based skip

The build system stamps all source files (mtime + size + FNV-1a hash) and stores the combined hash in `.mx/build_state.json`. If no stamps changed and the artifact exists, compilation is skipped.

### Build flow

1. Resolve entry point from manifest
2. Collect all `.mod`/`.def` files from include paths
3. Stamp all source files + manifest + lockfile
4. Compare combined hash against cached state
5. If changed: build `CompileOptions` from manifest, call `driver::compile()`
6. Save new build state

## Testing strategy

### Unit tests (cargo test)

230+ tests covering:
- Lexer: tokenization, keywords, case sensitivity, feature pragmas
- Parser: AST construction for various constructs
- CodeGen: `#line` directive emission, debug/non-debug mode
- LSP handlers: completion, hover, call hierarchy, signature help, highlighting
- Workspace index: call graph, incremental reindex, multi-root
- Analysis: scope map, reference index, symbol table

### Integration tests (tests/run_all.sh)

Categorized `.mod` files in `examples/` compiled and executed. Each test has an expected output comment at the top. The test runner compiles with `mx`, runs the binary, and compares stdout against expected output.

### Adversarial tests (tests/adversarial/)

1150+ tests across multiple compiler configurations (PIM4, M2+, optimized, with/without sanitizers, C/LLVM backends). Tests are defined in `tests/adversarial/tests.json` and run via `run_adversarial.py`. Use `--backend all` to test both C and LLVM backends.

### Conformance tests (tests/conformance.sh)

22 tests validating the compiler's external interface:
- `--version-json` output format
- `--print-targets` output
- `--emit-c` output
- `compile --plan` execution
- `--diagnostics-json` format
- Capability advertisement

### Running all tests

```bash
cargo test                                              # 230+ unit tests
bash tests/run_all.sh                                   # integration tests
bash tests/conformance.sh                               # conformance tests
python3 tests/adversarial/run_adversarial.py --mode ci  # adversarial tests (C backend)
python3 tests/adversarial/run_adversarial.py --backend all  # adversarial tests (C + LLVM)
```

## File reference

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point, subcommand routing |
| `src/lexer.rs` | Tokenizer |
| `src/parser.rs` | Recursive-descent parser |
| `src/ast.rs` | AST node types |
| `src/sema.rs` | Semantic analysis |
| `src/hir.rs` | HIR types (Place, Projection, HirExpr, HirStmt) |
| `src/hir_build/` | HIR builder: `mod.rs` (build_module, struct defs, tests) + `lower.rs` (HirBuilder impl) |
| `src/cfg/` | CFG: `mod.rs` (data model, verify, cleanup, DOT) + `build.rs` (CfgBuilder, lowering) |
| `src/codegen_c/` | C code generation (10 modules, CFG-driven control flow) |
| `src/codegen_llvm/` | LLVM IR code generation (12 modules, CFG-driven control flow, zero AST dependencies) |
| `src/target.rs` | Target abstraction (TargetInfo, layout, ABI) |
| `src/driver.rs` | Compilation orchestration |
| `src/build.rs` | Project build system |
| `src/analyze.rs` | Analysis-only path for LSP |
| `src/project_resolver.rs` | Manifest/lockfile parsing |
| `src/stdlib.rs` | Standard library definitions + C runtime |
| `src/symtab.rs` | Symbol table |
| `src/types.rs` | Type registry |
| `src/errors.rs` | Error types |
| `src/json.rs` | JSON parser |
| `src/lsp/server.rs` | LSP event loop and dispatch |
| `src/lsp/index.rs` | Workspace index |
| `src/lsp/analysis.rs` | Analysis caching, def cache |
| `src/lsp/documents.rs` | Open document store |
| `src/lsp/transport.rs` | JSON-RPC framing |
| `src/lsp/completion.rs` | Code completion |
| `src/lsp/hover.rs` | Hover information |
| `src/lsp/goto_def.rs` | Go to definition |
| `src/lsp/call_hierarchy.rs` | Call hierarchy |
| `src/lsp/highlight.rs` | Document highlight |
| `src/lsp/signature_help.rs` | Signature help |
| `src/lsp/symbols.rs` | Document symbols |
| `src/lsp/semantic_tokens.rs` | Semantic token highlighting |
| `src/lsp/code_actions.rs` | Code actions |
| `src/lsp/workspace.rs` | Re-exports from project_resolver |
