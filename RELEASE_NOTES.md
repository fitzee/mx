# Release Notes

## Library audit refactor (2026-03-13)

### Bug fixes

- **m2log 1.0.1** — Fix LogSinkStream importing from nonexistent `LogFmt` module; correct `Level.TRACE` to bare `TRACE`.
- **m2evloop 0.2.0** — Fix import shadowing of `Scheduler` type in Timers and EventLoop `.def`/`.mod` files, enabling qualified access (`Scheduler.Status`, `Scheduler.SchedulerEnqueue`).
- **m2oidc 0.1.3** — Return `JkFull` when JWKS key array overflows instead of silently dropping keys and returning `JkOk`.

### Features

- **m2bytes 1.2.0** — `AppendByte`, `AppendChars`, `AppendView` now return `BOOLEAN` (FALSE on allocation failure). Backward-compatible under PIM4.
- **m2stream 0.2.0** — Stream `.def` API updates.

### Hardening

- **m2evloop 0.2.0** — Timer ID counter wraps to 1 at `MAX(INTEGER)` instead of overflowing.
- **m2regex 0.1.1** — `FindAll` clamps output to caller's `matches` array capacity, preventing buffer overrun.
- **m2tls 0.1.1** — Promise lifetime alignment: `PromiseRelease` after every `Resolve`/`Reject`; consolidate settlement through `ResolveSess`/`RejectSess` helpers.
- **m2http 0.1.2** — Promise lifetime alignment in DNS, HTTPClient, H2Client.
- **m2rpc 0.1.2** — Promise lifetime alignment in RpcClient.
- **m2ws 0.1.2** — Promise lifetime alignment in WebSocket.

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
