# Release Notes

## 1.9.0 (2026-04-06)

### Features

- **LLVM backend: intrinsics** — The LLVM backend now emits native LLVM intrinsics instead of C runtime calls for key operations:
  - `@llvm.memcpy.p0.p0.i64` for record/struct assignment (enables LLVM to inline, vectorize, or elide dead copies)
  - `@llvm.fshl.i32` for ROTATE (single instruction on x86/ARM, replaces 4-instruction sequence)
  - `@llvm.sqrt/sin/cos/exp/log/atan.f32/f64` for MathLib functions (constant-folded, inlined to native FP instructions)
  - `@llvm.floor.f32/f64` for MathLib.entier (floor + fptosi, preserving PIM4 floor semantics)

### Bug fixes

- **LSP: false "undefined type" for qualified cross-module types** — The LSP reported spurious "undefined type 'Scheduler'" errors for types referenced via qualified access in transitive dependencies (e.g., `Scheduler.Scheduler` as a return type in EventLoop.def). The LSP analysis now uses two-pass type registration, matching the compiler driver.
- **LLVM backend: foreign module qualified imports** — `IMPORT Sys` of a `DEFINITION MODULE FOR "C"` failed to declare or name-map the C functions, causing "use of undefined value" errors at link time. Both qualified (`IMPORT M`) and unqualified (`FROM M IMPORT f`) foreign imports are now handled.
- **make install: missing Sys.def** — Fresh installs via `make install` did not copy `Sys.def` into `~/.mx/lib/m2sys/`, causing all projects that depend on m2sys to fail with "procedure not declared" errors.

### Tooling

- **VS Code: debug configuration provider** — The extension now registers a `DebugConfigurationProvider`, so m2dap appears in the VS Code debug sidebar when creating a new launch.json or pressing F5 without a config. Previously it only worked via the `mx: Create Debug Config` command.

## 1.8.3 (2026-04-03)

### Bug fixes

- **C backend: function call as array index** — Array subscript expressions using a function call on the LHS (e.g., `buf[GetPos()] := ch`) were silently emitted as index 0, writing to the wrong element. The designator expression emitter now delegates unhandled HIR expression kinds to the full expression emitter.
- **LLVM backend: foreign module qualified imports** — `IMPORT Sys` of a `DEFINITION MODULE FOR "C"` failed to declare or name-map the C functions, causing "use of undefined value" errors at link time. Both qualified (`IMPORT M`) and unqualified (`FROM M IMPORT f`) foreign imports are now handled.
- **ADR on single-char string literals** — `ADR("r")` emitted invalid code on both backends (scalar char instead of addressable pointer). String literals in ADR context are now emitted as interned constants.
- **Sema: VAR ADDRESS accepts typed pointers** — `ALLOCATE`/`DEALLOCATE` take `VAR ADDRESS` but were rejecting typed pointer arguments (e.g., `POINTER TO INTEGER`). The VAR parameter check now allows pointer↔ADDRESS compatibility.
- **Sema: OBJECT types assignable to REFANY** — M2+ OBJECT types are reference types but `is_ref()` did not include them, causing "incompatible type" errors when passing objects to REFANY parameters.
- **Sema: module/type name collision** — When a type and its module share the same name (e.g., `FROM Stream IMPORT Stream`), qualified calls like `Stream.Destroy()` were rejected as "field access on non-record type". The field-access fallback now tries qualified module lookup.

### Test coverage

- **func_call_index** — Adversarial regression test for function calls used as array indices on both backends.

## 1.8.2 (2026-04-01)

### Features

- **Procedure call argument type checking** — The semantic analyzer now verifies that actual parameter types are compatible with formal parameter types in procedure and function calls. Previously only argument count was checked.
  - Open array parameters require matching element types (e.g., `ARRAY OF INTEGER` rejected for `ARRAY OF CHAR`)
  - VAR parameters require type-identical arguments
  - Value parameters use assignment compatibility rules
  - Errors are reported both at compile time and in the LSP (real-time squiggles in VS Code)

### Bug fixes

- **StringLit assignment compatibility** — String literals are now mutually compatible in `assignment_compatible`, fixing false positives when passing string literals to stdlib procedures with `TY_STRING` formal parameters.

## vscode-m2plus 0.3.0 (2026-04-01)

### Features

- **Auto-capitalize keywords** — Keywords and builtins are automatically uppercased as you type. Triggers on word boundaries (space, semicolon, newline, etc.) and skips strings and comments.
- **Auto-import stdlib completions** — Typing a stdlib procedure name (e.g., `WriteString`, `Assign`, `sqrt`) offers a completion that also inserts the `FROM Module IMPORT ...;` line. Appends to existing imports from the same module.
- **Lazy LSP startup** — The language server now only starts when a `.mod`/`.def` file is opened. Commands like Initialize Project and Create Debug Config work immediately without waiting for the LSP.
- **Command activation** — All commands (init, restart, reindex, diagnose, debug config) now activate the extension on invocation, so they work even without a Modula-2 file open.

## 1.8.1 (2026-04-01)

### Bug fixes

- **DWARF local variable debug info** — The LLVM backend now emits `#dbg_declare` for local variables, not just parameters. Variables appear in the debugger inspector when building with `-g`.
- **LLVM optnone in debug builds** — Functions compiled with `-g` now include `optnone noinline` attributes, preventing LLVM's mem2reg pass from promoting allocas and dropping debug variable info.
- **Const eval wrapping arithmetic** — Integer constant evaluation uses wrapping add/sub/mul, fixing a panic on expressions like `4000000000000000H * 2` that overflow signed i64 (legitimate for CARDINAL/LONGCARD bit patterns).
- **CHR/ORD const eval** — Fixed CHR and ORD handling in constant expression evaluation in sema.
- **LLVM single-char string memcpy** — Fixed LLVM codegen for single-character string assignments.

### Tooling

- **m2dap 0.2.0: pty-based lldb** — The debug adapter now spawns lldb via a pseudo-tty (forkpty) so lldb flushes prompts immediately. Fixes the hang where lldb buffered stdout with plain pipes.
- **m2dap: sentinel match fix** — Dropped the `\n` prefix from the `(m2dap) ` prompt sentinel. When pty echo is suppressed, no newline precedes the prompt — the old sentinel never matched, causing a hang.
- **m2dap: full source paths in stack frames** — Stack traces now report absolute file paths via `settings set frame-format` with `${line.file.fullpath}`, so VS Code can open source files from the call stack.

## 1.8.0 (2026-03-31)

### Features

- **CFG v2: CFG-driven code emission** — Both C and LLVM backends now emit procedure and init bodies by iterating CFG basic blocks and terminators instead of reconstructing structured control flow. The CFG builder handles all control flow constructs including FOR, REPEAT, CASE, TRY, LOCK, and short-circuit booleans.
- **CFG construction in driver** — CFGs are built in a new driver Phase 4b and stored on HIR nodes (`HirProcDecl.cfg`, `HirModule.init_cfg`), separating control flow analysis from code emission.

### Build system

- **Module splits** — `cfg.rs` split into `cfg/mod.rs` + `cfg/build.rs`; `hir_build.rs` split into `hir_build/mod.rs` + `hir_build/lower.rs`. Both backends gain `cfg_emit.rs` for CFG-driven emission.

### Test coverage

- **FOR char literal** — Adversarial codegen test for FOR loops over character literals.

## 1.7.1 (2026-03-30)

### Bug fixes

- **Linux: link pthreads** — Add `-lpthread` on Linux targets. The runtime uses pthreads for Thread/Mutex/Condition support.
- **Linux: enable POSIX features** — Pass `-D_GNU_SOURCE` via `target.default_cflags()` on Linux. Enables `CLOCK_MONOTONIC`, `struct timespec`, and other POSIX features for all C files including extra-c sources.
- **LLVM backend: require clang 15+** — Emit clear error when clang version is too old for opaque pointer (`ptr`) support. The C backend works with any C compiler.

### Build system

- **Platform flags centralized in TargetInfo** — `default_cflags()` and `default_ldflags()` replace scattered platform-specific flag logic in the driver.
- **Makefile: clang version check** — `make install` reports clang version and LLVM backend compatibility during dependency check.

## 1.7.0 (2026-03-30)

### Features

- **Target abstraction (`--target`)** — New `TargetInfo` layer formalizes platform semantics (triple, arch, OS, pointer size, ABI, type layout, alignment). Constructed once at compile start; both backends use it for target-specific output. Supports `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `aarch64-darwin`. C backend emits `_Static_assert` layout guards.
- **Control flow graph (`--cfg`)** — New `src/cfg.rs` module builds CFGs from HIR. V1 supports linear statements, IF/ELSIF/ELSE, WHILE, LOOP, EXIT, RETURN, and short-circuit AND/OR/NOT. `mx --cfg program.mod -o program.dot` emits DOT graph with one subgraph per procedure.

### Bug fixes

- **Exception alias for .def module exceptions** — Emit `#define M2_EXC_Name Module_Name` for exceptions declared in definition modules. Fixes `make build` failure for mxpkg on fresh clone.
- **Compiler warnings eliminated** — Remove duplicate Array match arm and unused `mut` binding.

### Libraries

- **m2log 1.1.0** — API changes to Log.def.
- **m2sys 0.2.0** — API changes to Sys.def.
- **m2cli 0.1.1** — Implementation fixes.

## 1.6.0 (2026-03-30)

### Features

- **LLVM backend fully decoupled from AST** — All codegen reads from prebuilt HIR (`HirModule`, `HirProcDecl`, `HirEmbeddedModule`). Zero AST node data is accessed during code generation. The only AST import remaining is for shared types (`BinaryOp`, `UnaryOp`, `ExprKind`) used by HIR expressions.
- **Short-circuit AND/OR evaluation** — Both backends now correctly short-circuit boolean AND/OR. The C backend wraps operators in parentheses; the LLVM backend emits conditional branches with phi nodes.
- **COMPLEX/LONGCOMPLEX type support** — Variables use `m2_COMPLEX` struct type. Arithmetic operations delegate to runtime helpers. LLVM backend implements CMPLX/RE/IM as inline `insertvalue`/`extractvalue`.
- **Deep nested procedure closures** — Recursive `build_nested_recursive` in hir_build supports arbitrary nesting depth. Transitive capture propagation forwards grandchild captures through parent env structs.
- **Procedure-level EXCEPT handlers** — C backend wraps procedure bodies in M2_TRY/M2_CATCH when HirProcDecl has an except_handler.
- **TRY/FINALLY on exception path** — LLVM backend runs FINALLY handler before re-raising on the exception path.
- **Multidimensional array support** — Multi-index `A[i, j]` splits into separate Index projections per dimension. Array typedefs use `field_type_and_suffix` for correct 2D emission.
- **PIM4 floored DIV/MOD** — LLVM backend calls `m2_div`/`m2_mod` runtime helpers for signed integer division instead of LLVM's truncated `sdiv`/`srem`.
- **RTTI type descriptors** — C backend emits `M2_TypeDesc` globals for `Type::Ref` and `Type::Object`, enabling TYPECASE runtime dispatch.

### Bug fixes

- **Variant record codegen** — Skip synthetic `variant` pseudo-field and tag field from C struct emission. Fix variant field access paths (`._variant._vN.field`). LLVM: correct variant field GEP offsets, pseudo-field skip in both `type_lowering` and `llvm_type_for_type_id` paths.
- **Opaque type revelation** — Create `Type::Alias` instead of cloning target type data when implementation module reveals an opaque type. Ensures both names resolve to the same C/LLVM type.
- **Alias resolution** — Resolve aliases in CASE, FOR, WITH, DIV/MOD type checks and `get_ordinal_range`. Fixes enum-indexed array sizes, CASE on enum types, FOR on named types.
- **Constant forward references** — Re-evaluate constants after all declarations in a block, resolving forward-referenced constants like `Total = Base + Extra`.
- **Nested WITH** — Chain through parent WITH scope for nested `WITH p DO ... WITH birthdate DO ... year`.
- **Last-import-wins** — Allow re-importing the same name from a different module (PIM4 shadowing).
- **String-to-char-array overflow** — Use `m2_Strings_Assign` instead of `memcpy(dst, src, sizeof(dst))` to prevent buffer overread.
- **Single-char string constants** — LLVM: load first byte (`load i8, ptr`) instead of `ptrtoint` for string-to-char coercion. C: keep string form in `m2_Strings_Assign` calls.
- **Nested proc collision** — Use parent proc context to disambiguate same-named nested procedures (e.g., Alpha.Helper vs Beta.Helper).
- **LLVM double-to-float coercion** — Insert `fptrunc` when passing LONGREAL to REAL parameters.
- **Named array param dereference** — LLVM: load pointer from alloca before GEP for by-value array parameters.
- **Closure capture in C backend** — Search nested procs at any depth for capture analysis. Compute env_access_names with transitive captures.
- **Exception declarations** — Emit `#define M2_EXC_*` in gen_program_module and gen_implementation_module.
- **Suppress -Wunused-parameter** — Emit `(void)param;` for all parameters in generated C.
- **Compiler warnings** — Remove unused `mut` and unreachable patterns that leaked to stderr and caused false test failures.

### Architecture

- **1,672 lines of dead AST code deleted** from LLVM backend: gen_proc_decl, gen_type_decls, gen_const_decls, gen_var_decls_global, gen_var_decl_local, gen_exception_decls, count_stmts, legacy module methods, closures.rs, TypeNode functions.
- **HirProcDecl.body populated** for all procedures at all nesting depths (main module, local modules, embedded modules, deeply nested procs).
- **HirProcDecl.closure_captures populated** via `collect_hir_var_refs` with upward propagation for transitive captures.

### Test coverage

- Adversarial tests: **1147/1151 passing (100% non-skipped)**, up from 924/1151 (80.3%). 223 test failures fixed across both backends.

## 1.5.0 (2026-03-28)

### Features

- **Prebuilt HirModule** — `build_module()` constructs a complete HirModule as a distinct pipeline phase after sema, containing structural declarations (types, consts, globals, proc signatures, embedded modules) and pre-lowered statement bodies. Both C and LLVM backends iterate from HirModule for all structural emission.

- **TypeId → C type resolver** — Context-aware `type_id_to_c()` resolves TypeIds to C typedef names using a `typeid_c_names` map populated from HirModule type_decls, def-module registration, and gen_type_decl emission. Delegates to `named_type_to_c` for module-dependent name prefixing. Handles all type kinds including records, enums, pointers, arrays, procedure types, and sets.

- **AST bridge removal** — All `ast_type_node` and `ast_return_type` bridge fields removed from HIR types. Procedure prototypes, global variable declarations, type declarations, and record forward declarations all use pure TypeId resolution. HIR + sema are the only contract between frontend and backend.

### Bug fixes

- **Scoped symtab lookups** — `build_module()` uses `lookup_module_scope` + `lookup_in_scope_direct` instead of `lookup_any` for type, const, var, and proc extraction. Prevents cross-module TypeId conflicts when names collide across scopes.

- **Import AS alias resolution** — Fixed native stdlib arg stripping for import aliases in M2+ modules.

- **FOR BY step direction** — Fixed downward FOR loops with negative step producing wrong direction comparison.

- **WITH scope lookup** — Fixed `lookup_any` returning wrong type for common names in WITH scope resolution.

### Test coverage

- **Main module types adversarial test** — Exercises all type declaration kinds (Record, Enum, Pointer-to-Record, Array, ProcedureType, Set, Subrange, Alias) in a program module body.

## 1.4.1 (2026-03-26)

### Bug fixes

- **Def module dependency ordering** — Phase 3 of the driver now recursively registers `.def` dependencies in correct order, fixing cases where imported types (e.g., `URIRec` from `URI.def`) resolved as `TY_VOID` when their defining module was loaded after the module that imports them.
- **TSIZE type name mangling** — `TSIZE(RecordType)` inside embedded implementation modules now correctly emits the module-prefixed C name (e.g., `Jwks_KeySetRec` instead of bare `KeySetRec`).
- **POINTER TO ARRAY declarations** — Inline `POINTER TO ARRAY [lo..hi] OF T` variables now declare as `T (*name)[size]` in C instead of `T *name`, fixing `(*p)[i]` dereferencing.
- **Procedure-local variable shadowing** — Local variables that shadow module-level names no longer get incorrectly module-prefixed in C declarations.
- **LLVM aggregate assignment with float fields** — Record assignment for structs containing `REAL` fields now correctly uses memcpy instead of an invalid scalar store.
- **LLVM open array high forwarding** — Forwarding an open array parameter to another procedure now passes the correct `_high` bound instead of 0.
- **Struct by-value arguments** — Fixed passing records by value in function calls (ADR type, optnone for large functions).
- **Indirect calls through procedure variables** — Fixed parameter info lookup and open array expansion for calls through procedure-typed variables.
- **Module.Proc qualified calls** — Fixed `lookup_proc_params` for qualified procedure calls.
- **Record field TypeId tracking** — Sema fixup and type-lowering-first approach for correct GEP indices.
- **Array type resolution** — Prefer canonical sizes from sema TypeIds over LLVM type strings.
- **NEW allocation size** — `NEW(p)` now allocates the actual pointed-to type size instead of a hardcoded 256 bytes.
- **TSIZE in LLVM** — Computes actual type size via GEP-from-null instead of hardcoded values.
- **Named array param detection** — Per-function tracking prevents cross-procedure interference.

### Architecture

- **HIR pipeline** — New shared intermediate representation (`src/hir.rs`, `src/hir_build.rs`) used by both C and LLVM backends for designator resolution, open array expansion, and local/global identity. Sema scope chain is now the single source of truth for variable locality and type identity.
- **C backend split** — `src/codegen.rs` split into 8 focused modules under `src/codegen_c/`: mod, modules, decls, stmts, exprs, designators, types, m2plus.

## 1.4.0 (2026-03-23)

### Features

- **LLVM: stack traces** — Unhandled exceptions and HALT now print a full stack trace with procedure names, source files, and line numbers. Lightweight frame tracking via thread-local `m2_StackFrame` stack — procedure entry pushes a frame, exit pops it, each statement updates the line number. No external dependencies, no DWARF parsing required, works with and without `-g`.

## 1.3.1 (2026-03-23)

### Optimizations

- **LLVM: function attributes** — `nounwind` on all non-exception procedures, `noalias nocapture` on VAR params, `nocapture readonly` on non-VAR open array and named array params, `noundef` on scalar value params.
- **LLVM: canonical FOR loops** — Preheader/header/latch/exit structure with empty-range guard. Exit test after increment (latch-exit pattern). `nsw` on induction variable increment. Enables IndVarSimplify and LoopVectorize.
- **LLVM: PHI-based short-circuit AND/OR** — Eliminates non-entry-block allocas. LLVM collapses nested boolean chains to branchless `and i1` / `or i1` sequences.
- **LLVM: inline m2_div/m2_mod** — Floored division and modulo emitted as inline IR instead of opaque C runtime calls. Enables constant folding (`DIV 2` → `ashr 1`), strength reduction, and `sdiv` → `udiv` promotion when LLVM proves positive range.
- **LLVM: runtime function attributes** — `readnone nounwind willreturn` on m2_div/m2_mod/m2_div64/m2_mod64. `nounwind readonly` on strcmp/strlen. `noalias` on malloc. `nocapture` on free.
- **LLVM: BOOLEAN !range metadata** — Loads of BOOLEAN variables annotated with `!range !{i32 0, i32 2}`, enabling value-range optimizations.
- **LLVM: unreachable dead blocks** — Dead code after return/exit emits `unreachable` instead of dangling `ret`.
- **LLVM: current_block tracking** — `emitln` auto-tracks the current basic block label for correct PHI node predecessor references in nested expressions.
- **LLVM: fn_return_types map** — Cross-module function call return types resolved via `gen_proc_decl`-populated map, fixing void-return miscompilation for embedded module calls.

## 1.3.0 (2026-03-23)

### Features

- **Native M2 stdlib** — Both C and LLVM backends now compile 13 stdlib modules (InOut, Strings, Storage, Terminal, MathLib, RealInOut, FileSystem, BinaryIO, STextIO, SWholeIO, SRealIO, SLongIO) from native Modula-2 source instead of hardcoded C functions. Single source of truth for stdlib behavior across backends.
- **Slimmed runtime header** — C backend runtime header reduced from ~660 to ~380 lines. Dead C stdlib functions removed; only core runtime (exception handling, RTTI, div/mod, complex arithmetic, threads, Args) remains.
- **Library root includes** — `find_def_file` and `find_mod_file` now scan `~/.mx/lib/*/` package roots in addition to `*/src/` subdirectories.

### Bug fixes

- **LLVM: cross-module function return types** — Function return types are now tracked in a dedicated `fn_return_types` map populated by `gen_proc_decl`, fixing void-return calls to functions like `MathLib.Entier` from embedded modules.
- **LLVM: string CONST open array passing** — Local string constants (CONST s = "...") passed to open array parameters now correctly load the string pointer and compute HIGH from the string length, instead of passing the address of the pointer global with HIGH=0.
- **LLVM: duplicate global variables** — Globals emitted from both definition and implementation modules are now deduplicated, preventing LLVM IR redefinition errors.
- **LLVM: case-insensitive stdlib name resolution** — Import-site casing (e.g. `Entier`) is resolved against the def-file casing (e.g. `entier`) for native stdlib modules via case-insensitive `declared_fns` lookup.
- **LLVM: native Strings calling convention** — Bypass the old `gen_strings_call` special handler for native Strings module; `gen_call` handles open array params correctly.
- **LLVM: `map_stdlib_call` bypass** — Native stdlib functions no longer go through `map_stdlib_call` which returned C expressions (like `(int32_t)floorf`) invalid as LLVM IR function names.
- **C backend: open array param name mangling** — `open_array_params` now stores mangled names, fixing HIGH computation for parameters whose names are C keywords (e.g. `default`).
- **C backend: skip `get_stdlib_proc_params` for native modules** — Prevents hardcoded param info (without open array flags) from overwriting the actual param info from compiled modules.
- **mxpkg0: respect dep `includes` field** — `resolve_deps` now reads the dependency's `m2.toml` `includes` field instead of always assuming `src/`, fixing resolution for packages like m2sys with `includes=.`.

### Build system

- **m2fmt.c linking** — Float formatting helpers (`m2fmt.c`) are now linked in both C and LLVM backend compile paths, and auto-detected by the adversarial test runner.

## 1.2.0 (2026-03-23)

### Features

- **LLVM backend** — New `--llvm` flag emits LLVM IR and compiles with clang. Native DWARF debug info with M2 type names, full variable inspection in lldb. Set `backend=llvm` in `m2.toml` for project builds.
- **`--emit-llvm`** — Emit `.ll` text files without compiling. Separate from `--llvm` which does the full compile+link.
- **LLVM-native exception handling** — TRY/EXCEPT/FINALLY uses `invoke`/`landingpad` with a custom `m2_eh_personality` function. Full LSDA parsing (call site table, action table, type table) in the C runtime. Nested TRY blocks in the same function propagate correctly via `_Unwind_Resume`.
- **SjLj exception handling** — ISO procedure-level and module-body EXCEPT uses setjmp/longjmp. Coexists with native EH: RAISE inside a SjLj-guarded procedure uses `m2_raise`, inside TRY uses `m2_eh_throw`.
- **RTTI** — `M2_TypeDesc` globals for REF/OBJECT types, `M2_RefHeader` prepended to typed allocations, `M2_ISA` for TYPECASE runtime type checking, `M2_ref_alloc`/`M2_ref_free` for typed allocation.
- **ADDRESS^[i] byte-level indexing** — m2plus extension: dereference ADDRESS and index as `ARRAY OF CHAR`. C backend emits `((unsigned char*)ptr)[i]`, LLVM backend emits GEP on i8.
- **m2dap 0.1.0** — Modula-2 Debug Adapter Protocol server. Wraps lldb as a subprocess, translates DAP messages, formats variables with M2 type names and values (TRUE/FALSE, NIL, CHR(N), demangled procedure names).
- **VS Code m2dap integration** — New `m2dap` debug adapter type in the extension. `mx.m2dapPath` setting. "Create Debug Configuration" generates both m2dap and CodeLLDB launch configs.
- **Canonical Sys.def** — m2sys now ships a `DEFINITION MODULE FOR "C" Sys` with all FFI bindings. Libraries use `m2sys` as a dependency instead of copying Sys.def and linking m2sys.c directly.
- **Function call dereference** — `Func(x)^` now parses and codegens correctly. Previously rejected by the parser.
- **LONGREAL D/d exponent** — The lexer accepts `D` and `d` as exponent suffixes for LONGREAL literals (e.g., `1.0D2`).

### Bug fixes

- **FOR control variable assignment** — Removed the restriction preventing assignment to FOR control variables inside the loop body. The PIM4 spec does not mandate this check, and it broke valid programs.
- **Nested TRY propagation** — Inner TRY with no matching handler now resumes to outer TRY via a `try.nomatch` label with `_Unwind_Resume`, instead of falling through to normal execution.
- **FINALLY cleanup landing pads** — FINALLY-only TRY blocks use `catch ptr null` (catch-all) so the search phase finds a handler. Previously cleanup-only landing pads were skipped, causing "Unhandled exception" for same-function nested TRY.
- **Personality function declaration** — `@m2_eh_personality` is now declared eagerly when m2plus adds the personality attribute to a function, instead of waiting for a TRY statement.
- **DWARF debug records** — Switched from deprecated `call @llvm.dbg.declare` to LLVM 19+ `#dbg_declare` records. Variables now appear in lldb `frame variable`.
- **DWARF language tag** — Changed from `DW_LANG_Modula2` to `DW_LANG_C99` so lldb's C type system can inspect variables (lldb has no Modula-2 language plugin).
- **m2http H2Client/HTTPClient** — Fixed `WritePreface` to use `AppendChars` instead of byte-by-byte `AppendByte`.

### Tooling

- **mxpkg 0.1.1** — Builder.mod rewritten as a thin wrapper around `mx build`/`mx run`, replacing ~580 lines of duplicated build logic.
- **Adversarial test runner** — New `--backend c,llvm,all` flag. LLVM tests compile with `mx --llvm`, support `skip_llvm` and extra C files. 1100+ tests across both backends.

### Test coverage

- New adversarial suites: `address_index`, `cross_module_name_clash`, `param_entry_clash`, `record_cross_module`, `record_param_cross`, `stdlib_args`, `try_except_basic`, `try_except_nested`, `typecase_basic`, `typecase_object`, `except_handler`, `finally_cleanup`.

### Documentation

- README updated for dual-backend architecture and m2dap.
- `docs/architecture.md` — LLVM backend pipeline, codegen_llvm module table, design decisions.
- `docs/toolchain.md` — `--llvm`/`--emit-llvm` flags, backend comparison table, m2dap section.
- `docs/vscode.md` — m2dap vs CodeLLDB comparison, `mx.m2dapPath` setting, updated debugging guide.
- `docs/faq.md` — "Why two backends?" replaces "Why transpile to C?", new m2dap FAQ entry.

## 1.1.1 (2026-03-15)

### Bug fixes

- **Enum-indexed array codegen** — `ARRAY EnumType OF T` declarations emitted zero-size C arrays, causing stack corruption. Now correctly emits `[m2_max_EnumName + 1]`.
- **LSP def cache m2plus threading** — The LSP's definition file cache did not pass the `m2plus` flag to the lexer, causing false parse errors on `.def` files using M2+ syntax (e.g., import `AS` aliases) in M2+ projects.
- **EXIT in all loop forms** — EXIT is now valid inside WHILE, REPEAT, and FOR loops, not just LOOP. Previously produced false "EXIT must be inside a LOOP" sema errors.

### Features

- **MathLib.Random** — New `Random(): REAL` returns a pseudo-random value in [0.0, 1.0).
- **MathLib.Randomize** — New `Randomize(seed: CARDINAL)` seeds the PRNG.
- **Strings.CAPS** — New `CAPS(VAR s: ARRAY OF CHAR)` converts a string to upper case in place.

## 1.1.0 (2026-03-14)

### Features

- **PIM4 strict keyword gating** — 18 M2+ keywords (TRY, EXCEPT, FINALLY, RAISE, RETRY, AS, BRANDED, EXCEPTION, LOCK, METHODS, OBJECT, OVERRIDE, REF, REFANY, REVEAL, SAFE, TYPECASE, UNSAFE) are now only recognized as keywords when `--m2plus` is enabled. In default PIM4 mode, they are valid identifiers.
- **FOR control variable protection** — Assignment to a FOR loop control variable inside the loop body is now a semantic error, per PIM4 specification.
- **RETURN validation** — Function procedures that omit the return expression, and proper procedures that include one, now produce semantic errors.
- **Set constructor typing** — Typed set constructors (e.g., `CharSet{0C..37C}`) now resolve to their declared type instead of always defaulting to BITSET.

### Documentation

- **Grammar reference rewrite** — `docs/lang/grammar.md` restructured into three sections: PIM4 Core (with correct operator precedence, terminal definitions, and Definition production), mx Accepted Differences, and Modula-2+ Extensions.
- **PIM4 conformance audit** — New `docs/PIM4_CONFORMANCE_AUDIT.md` with 27 findings covering parser, sema, type system, grammar doc, and extension gating.

### Test coverage

- 32 new unit tests covering: M2+ keyword-as-identifier in PIM4 mode, extension syntax rejection/acceptance, RETURN edge cases, FOR variable assignment, set constructor typing, and LSP/CLI parity.

## 1.0.6 (2026-03-14)

### Bug fixes

- **64-bit DIV/MOD truncation** — `m2_div`/`m2_mod` (int32_t) silently truncated LONGCARD/LONGINT operands. New `m2_div64`/`m2_mod64` helpers, plus type tracking for procedure parameters and type aliases, ensure correct width selection.
- **Def-only module types emitted before embedded implementations** — Pure type/constant modules now emit types in the C preamble before embedded modules.
- **Array-indexed field resolution** — `arr[i].field` assignments now resolve through tracked array element types.
- **FOR loop bound expression precedence** — FOR loop start/end expressions now use `gen_expr_for_binop`, preventing incorrect C operator precedence when bounds contain binary operations.
- **LSP transitive import resolution** — The language server now loads transitive `.def` dependencies (e.g., if module A imports B which imports C, C is now visible). Previously only direct imports were loaded, causing false "undefined type" diagnostics.
- **LSP def module registration order** — Loaded `.def` modules are registered in dependency-first order so types from transitive imports resolve correctly during semantic analysis.
- **Registry deps resolved in project resolver** — `DepSource::Registry` dependencies are now resolved via installed paths instead of being silently skipped, fixing include path resolution for registry-sourced packages.
- **m2log 1.0.1** — Fix LogSinkStream importing from nonexistent `LogFmt` module.
- **m2evloop 0.2.0** — Fix import shadowing of `Scheduler` type; timer ID wraps instead of overflowing.
- **m2oidc 0.1.3** — Return `JkFull` on JWKS key array overflow.
- **m2regex 0.1.1** — `FindAll` clamps output to caller's array capacity.
- **Promise lifetime alignment** — m2tls 0.1.1, m2http 0.1.2, m2rpc 0.1.2, m2ws 0.1.2.

### Features

- **m2metrics 0.1.0** — New library: system metrics (load average, memory, CPU time, process RSS).
- **m2lmdb 0.2.0** — New `DbiStatEntries` procedure.
- **m2bytes 1.2.0** — `AppendByte`, `AppendChars`, `AppendView` now return `BOOLEAN`.
- **m2stream 0.2.0** — Stream `.def` API updates.
- **Strings.Assign constant-folding** — `m2_Strings_Assign` is now `always_inline` with `__builtin_*` intrinsics, enabling compile-time constant folding when source length and destination capacity are known.

### Test coverage

- New adversarial suites: `longcard_div_mod`, `longint_div_mod`, `def_only_module`, `array_field_name_collision`.

### Documentation

- m2metrics library docs, library count updated to 33 across all docs.

## m2futures 0.2.0 (2026-03-12)

### Features

- **Promise/Future lifetime management** — New `PromiseRelease` and `FutureRelease` procedures for explicit handle ownership. `PromiseCreate` returns an alias pair sharing one reference; callers release exactly one.
- **Cancel token safety** — `CancelTokenDestroy` is now safe to call immediately after `Cancel`. Dispatch holds its own internal reference via a `dispatching` flag and dispatch ref, preventing use-after-free when the scheduler has queued `ExecCancelCB`.
- **Future.Release** — The `Future` convenience module now re-exports `FutureRelease` as `Release`.

### Bug fixes

- **Cancel dispatch use-after-free** — `Cancel` now acquires a dispatch reference before enqueuing `ExecCancelCB`, preventing the token pool slot from being freed while callbacks are still queued.
- **OnCancel double-dispatch** — `OnCancel` no longer resets `cbNext` or enqueues a second dispatcher while one is already in flight. New callbacks appended during dispatch are picked up naturally by the active dispatcher.
- **ExecCancelCB enqueue failure** — Dispatch reference is now released on scheduler enqueue failure, preventing token leaks.

### Documentation

- **Ownership model** — Definition module documents alias-pair semantics, double-release/leak rules, and per-chaining-output independent references.
- **All result pointer lifetime** — Documents that `AllResultPtr` is valid only until `FutureRelease` is called on the output future.
- **Best-effort combinator construction** — Documents partial-failure semantics for `All` and `Race`.
- **Cancellation limits** — Documents 8-callback-per-token limit and lossy scheduler-full behavior.

### Test coverage

- **New test suite** — 115 tests covering scheduler basics, promise lifecycle, settlement, chaining (Map/OnReject/OnSettle), combinators (All/Race), cancel tokens, OnCancel dispatch ordering, CancelTokenDestroy-after-Cancel safety, and MapCancellable flows.

## 1.0.3 (2026-03-12)

### Bug fixes

- **Definition module load order** — Def modules are now registered in topological (dependency-first) order, fixing "undefined type" errors when a def imports types from another def that hasn't been registered yet.
- **Re-export visibility** — Symbols imported into a `.def` module are now marked as exported, so qualified access (e.g., `Module.ImportedType`) works correctly from client code.
- **Module symbol shadowing** — `FROM Module IMPORT Module` (where type and module share a name) no longer prevents qualified access to other members of that module.

### Libraries

- **m2fmt 0.1.1** — PIM4-conformant `LONGCARD` pointer arithmetic in `PutCh`.
- **m2hash 0.1.1** — PIM4-conformant `LONGCARD` pointer arithmetic in `BucketAt`.
- **m2stream 0.1.1** — PIM4-conformant `LONGCARD` pointer arithmetic in `OffsetPtr`.
- **m2text 0.1.1** — PIM4-conformant `LONGCARD` pointer arithmetic in `PtrAt`.
- **m2alloc, m2http2** — Test-only fixes: `LONGCARD` pointer comparisons and arithmetic (no version bump).

## Library releases (2026-03-11)

### Libraries

- **PIM4 pointer arithmetic conformance** — Eliminated hardcoded array overlay types (`POINTER TO ARRAY [0..N] OF CHAR`) and standardized all pointer arithmetic on `LONGCARD` across 9 libraries. Overlay patterns replaced with `CharPtr = POINTER TO CHAR` plus `LONGCARD`-based address computation. Affected libraries: m2alloc, m2bytes, m2http, m2http2, m2json, m2oidc, m2rpc, m2tok, m2ws.
- **m2alloc 1.1.0** — Removed `ByteArray`/`BytePtr` exports from `AllocUtil.def`. `PtrAdd` offset parameter and `PtrDiff` return type changed from `CARDINAL` to `LONGCARD`. `FillBytes` rewritten with `CharPtr` arithmetic.
- **m2bytes 1.1.0** — Removed `BBufPtr` overlay from `ByteBuf.def`. Internal byte access uses `CharPtr` + `LONGCARD` arithmetic.
- **m2json 0.2.0** — Removed `SrcArray`/`SrcPtr` exports from `Json.def`. `Parser.src` field changed from `SrcPtr` to `ADDRESS`. All direct array indexing replaced with `PeekChar` helper.
- **m2http2server 0.2.0** — Increased `MaxReqValueLen` from 1023 to 8191 (8 KB) in `Http2ServerTypes.def` to accommodate full-size OIDC JWTs in HTTP/2 request headers.
- **m2oidc 0.1.2** — Restored PIM4-conformant `CopyN` using `CharPtr` + `LONGCARD` arithmetic. Fixed `ParseDiscovery` to drain nested JSON objects/arrays inline instead of calling `Json.Skip`, which could lose tokens.
- **m2pthreads 0.1.1** — Set explicit 2 MB stack size for spawned threads in `threads_shim.c` to avoid platform-dependent defaults.

## 1.0.2 (2026-03-10)

### Bug fixes

- **String literal assignment buffer overflow** — Assigning a short string literal to a larger `ARRAY OF CHAR` (e.g., `s := "hello"` where `s` is `ARRAY [0..31] OF CHAR`) generated `memcpy(dest, "hello", sizeof(dest))`, reading past the end of the literal. Now emits `memset` + bounded `memcpy` with the literal's actual size. Fixes AddressSanitizer global-buffer-overflow in all affected patterns (direct variables, record fields, nested records).
- **Multi-dimensional array constant bounds** — `ARRAY [1..N],[1..N] OF INTEGER` where `N` is a module constant generated `typedef int32_t Matrix[N + 1][N + 1]` before `N` was declared, producing a C compile error. Constant integer values are now evaluated in a pre-pass and inlined into array bound expressions, so the typedef emits `[3 + 1][3 + 1]` regardless of declaration order.

### Build system

- **Cargo workspace** — `cargo build --release` now builds both mx and mxpkg0 in a single invocation via a `[workspace]` section in the root Cargo.toml.
- **`make install` builds mxpkg** — The Makefile `build` target bootstraps the self-hosted mxpkg package manager after building mx and mxpkg0. `make install` copies all three binaries to `~/.mx/bin/`.
- **OpenSSL is a mandatory dependency** — `make check-deps` (run automatically by `make build`) detects OpenSSL on both macOS (Homebrew paths, pkg-config) and Linux (pkg-config, system headers), and fails early with platform-specific install instructions if missing.
- **mxpkg0 prefers release builds** — The bootstrapper now checks `target/release/mx` before `target/debug/mx`, and supports `MX` environment variable override. Also parses `[cc.feature.MACOS]` / `[cc.feature.LINUX]` manifest sections with automatic platform detection.

### Test coverage

- 150 cargo unit tests
- 883 adversarial tests across 8 compiler configurations (up from 558), including 40 new tests migrated from standalone examples covering: CASE range labels, DIV/MOD floor semantics, FOR..BY variants, subrange types, variant records, procedure types, open arrays, opaque types, import aliases, FFI bindings, closures, exceptions, and multi-dimensional arrays.

## 1.0.1 (2026-03-09)

### Bug fixes

- **Enum variant scope pollution in multi-module codegen** — Enum variant names (`OK`, `Invalid`, `OutOfMemory`, etc.) shared across different modules no longer collide. Previously, `import_map` entries leaked between embedded modules, causing variant names to resolve to the wrong source module. Each embedded module now starts with a clean import scope, and bare-key `enum_variants` entries are only registered for the main module.
- **Open array high bound missing in cross-module calls** — When multiple imported modules exported procedures with the same name (e.g., `Init`), calling the FROM-imported one with an open array parameter could omit the `_high` argument in the generated C. The symtab's `lookup_any` found the wrong module's procedure first, returning param info without the open array flag. FROM-import prefixed lookup now takes priority over bare-name symtab lookup, matching the existing FuncCall path.
- **Cross-platform build support** — Homebrew-specific include/library paths (`/opt/homebrew`) are now gated behind `[cc.feature.MACOS]` in library m2.toml files. The build system auto-injects `MACOS` or `LINUX` as implicit platform features at build time. The compiler driver gates GC paths and `-framework` flags on `cfg!(target_os = "macos")`. Libraries now build on Linux with system-installed packages (e.g., `libssl-dev`, `liblmdb-dev`) without extra flags.

## 1.0.0

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
