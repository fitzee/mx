# Release Notes

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
