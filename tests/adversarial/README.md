# Adversarial Test Suite for m2c

Stress tests for the m2c Modula-2 -> C transpiler, targeting:

- **Symbol namespace collisions** -- enum/type/constant name uniqueness in generated C
- **Semantic correctness** -- short-circuit, aliasing, init order, integer bounds
- **Name resolution** -- ambiguous imports, local shadowing, qualified access
- **Import chains** -- multi-hop wrapper chains, qualified procedure calls
- **Procedure values** -- proc vars, cross-module proc vars, same-name disambiguation
- **ABI / layout** -- record field access, nested arrays/records, VAR param passing
- **UB detection** -- ASan+UBSan on generated C code
- **Metamorphic testing** -- O0 vs O2 output equivalence, 4 source transforms
- **Fuzzing** -- parser crash testing, well-typed program generation (VAR params + records), corpus persistence
- **Runtime edge cases** -- EventLoop timers, Stream partial I/O
- **Strict ambiguity** -- emulated strict mode catches duplicate-import ambiguities
- **Stream stress** -- proc vars through complex selectors, pointer chains, parallel arrays

## Quick Start

```bash
# From repo root:
python3 tests/adversarial/run_adversarial.py --mode ci

# Local thorough run:
python3 tests/adversarial/run_adversarial.py --mode local

# Specific category:
python3 tests/adversarial/run_adversarial.py --category symbol_namespace

# Multiple categories:
python3 tests/adversarial/run_adversarial.py --category resolution,import_chain,proc_values

# Multi-TU mode (per-module .c files, separate compile + link):
python3 tests/adversarial/run_adversarial.py --link-mode multi_tu

# Strict ambiguity checking:
python3 tests/adversarial/run_adversarial.py --strict on --category strict_ambiguity

# Full CI with strict + sanitizers:
python3 tests/adversarial/run_adversarial.py --mode ci --sanitizers on --strict on
```

## Requirements

- Python 3.8+  (stdlib only, no pip dependencies)
- Rust toolchain (for `cargo run` to build m2c)
- At least one of: `clang`, `gcc`, or `cc`
- For runtime tests: OpenSSL development headers (`brew install openssl@3` on macOS)

## CLI Options

| Flag | Default | Description |
|------|---------|-------------|
| `--mode` | `ci` | `ci` = fast budget; `local` = thorough |
| `--category` | `all` | Comma-separated (see categories below) |
| `--compiler` | `all` | `clang`, `gcc`, or `all` |
| `--sanitizers` | `on` | `on` or `off` -- ASan+UBSan on generated C |
| `--link-mode` | `single_tu` | `single_tu` or `multi_tu` -- per-module C compilation + link |
| `--strict` | `off` | `on` or `off` -- emulated strict ambiguity checking |
| `--seed` | `20260221` | Deterministic seed for fuzz tests |
| `--config` | `config.json` | Path to config file |
| `--tests` | `tests.json` | Path to test catalog |

## Test Categories

### A) Symbol Namespace (`symbol_namespace`)

Tests that the generated C has no duplicate `typedef`, enumerator, or global symbol names
when multiple modules define types with the same name (e.g. `Status`).

| Test | What it catches |
|------|-----------------|
| `enum_collision` | Two modules with `TYPE Status = (OK, Error)` compiled into one TU |
| `stress_collision` | Six modules (EventLoop, Sockets, TLS, Promise, Scheduler, Stream) each defining `Status` |
| `unqualified_variant` | `s := OK` resolves correctly in single-module programs |
| `qualified_variant` | `QA.MakeOK()` resolves through correct module's enum |
| `mtu_linkage_trap` | Two modules with same-named exports (`value`, `GetValue`) -- proves multi-TU linkage is correct |

Each test also runs a **post-transpile C scan** that checks the generated `.c` file for
duplicate typedef names, enumerator names, and non-static function definitions.

An enhanced **missing-static scan** also flags non-static function definitions at file scope
(warning only, does not fail tests). This will be critical when multi-TU mode is implemented.

### B) Semantics (`semantics`)

Deterministic output-based tests for language semantics.

| Test | What it catches |
|------|-----------------|
| `short_circuit` | AND/OR must short-circuit (side-effect counter proves it) |
| `integer_bounds` | MAX/MIN(INTEGER), DIV/MOD on negatives, boundary arithmetic |
| `var_aliasing` | VAR param aliasing (same var passed twice), array element VAR params |
| `module_init_order` | Module body initialization runs in dependency order |

**Note:** SET type tests are not included (SET types not currently supported by m2c).

### C) Name Resolution (`resolution`)

Tests for correct name resolution: duplicate imports (last-import-wins), local shadowing.

| Test | What it catches |
|------|-----------------|
| `ambiguous_enum` | `FROM A IMPORT GetA; FROM B IMPORT GetA;` -- last import wins (returns 2) |
| `shadow_local` | Local `GetValue` var coexists with `Shadow_A.GetValue()` qualified call |
| `ambiguous_type` | `FROM A IMPORT Tag, GetTag; FROM B IMPORT Tag, GetTag;` -- last import wins |

### D) Import Chains (`import_chain`)

Tests multi-hop module dependencies and qualified access patterns.

| Test | What it catches |
|------|-----------------|
| `reexport_chain` | 3-hop chain: A->B->C->Main, each wrapping the previous module's proc |
| `qualified_access` | Two modules with same proc names, disambiguated via `IMPORT M; M.Proc(...)` |

### E) Procedure Values (`proc_values`)

Tests procedure-typed variables, higher-order functions, and cross-module proc vars.

| Test | What it catches |
|------|-----------------|
| `proc_var_basic` | Assign local procs to vars, call through vars, higher-order `Apply` |
| `proc_var_cross` | Import procs from module, store in array of proc vars, call through array |
| `proc_var_collision` | Two modules export `Calc`, assigned to separate proc vars via qualified names |

### F) ABI / Layout (`abi_layout`)

Tests record field access, nested compound types, and VAR parameter passing of records.

| Test | What it catches |
|------|-----------------|
| `record_fields` | Multi-type record (INTEGER, BOOLEAN, CHAR), VAR param passing, field access |
| `nested_compound` | ARRAY OF RECORD, RECORD with ARRAY fields, nested field/element access |

### G) UB / Sanitizer (`ub_sanitizer`)

Programs that SHOULD be safe -- if ASan/UBSan triggers, the codegen has a bug.

| Test | What it catches |
|------|-----------------|
| `array_bounds_safe` | Array fill + sum + boundary access -- safe indices only |
| `signed_overflow` | Integer arithmetic that stays within 32-bit range |

### H) Metamorphic (`metamorphic`)

Verifies the same program produces identical output across:
- `-O0` vs `-O2` compilation
- 4 source-to-source transforms:
  - **dead_code** -- insert `IF FALSE THEN ... END` after BEGIN
  - **alpha_rename** -- rename single-letter variables to `zz_` prefixed names
  - **decl_reorder** -- reverse order of top-level PROCEDURE declarations
  - **temp_intro** -- introduce a temp variable for the first `WriteInt(Func(...), w)` call

### I) Fuzzing (`fuzz`)

Time-bounded, seeded, reproducible:

1. **Corpus replay** -- Previously-failing inputs are re-tested; passing ones auto-removed
2. **Parser crash fuzzer** -- random token sequences; transpiler may reject but must not crash
3. **Well-typed program fuzzer** -- generates valid M2 programs with:
   - Integer arithmetic
   - VAR parameter procedures (50% chance)
   - Swap procedures (30% chance)
   - Record types with field access (40% chance)

Failing inputs are saved to:
- `out/<timestamp>/fuzz/failures/` -- per-run artifacts
- `fuzz_corpus/parser/` and `fuzz_corpus/typed/` -- persistent corpus for regression

Budget: CI = 50 parser + 20 typed inputs (30s cap); Local = 1000 + 200 (180s cap).

### J) Runtime (`runtime`)

Network/EventLoop tests over loopback TCP. **Marked `local_only`** -- skipped in CI mode.

| Test | What it catches |
|------|-----------------|
| `timer_cancel` | Timer creation, firing, cancellation via EventLoop |
| `stream_partial` | Partial read/write reassembly over loopback TCP |

### K) Strict Ambiguity (`strict_ambiguity`)

Emulated strict mode -- only runs when `--strict on`. The runner parses `FROM M IMPORT` lines
and detects names imported from 2+ modules that are used unqualified. Tests are marked with
`"strict": true` and `"expect_compile_fail": true`.

| Test | What it catches |
|------|-----------------|
| `ambig_type` | `Status` imported from both `SA_A` and `SA_B`, used bare as type |
| `ambig_enum` | `State` and `OK` imported from both `SE_A` and `SE_B`, used bare |
| `ambig_proc` | `Init` imported from both `SP_A` and `SP_B`, assigned to proc var |
| `reexport_ambig` | `Compute` re-exported through `RA_A` and `RA_C`, used bare |

### L) Stream Stress (`stream_stress`)

CI-safe, deterministic, no networking. Targets historically fragile codegen paths: procedure
references through complex selectors, pointer chains, parallel arrays of records + proc vars.

| Test | What it catches |
|------|-----------------|
| `callback_dispatch` | Array of proc vars called with record field arguments |
| `pointer_chain` | Linked list traversal + indexed proc var call on `p^.id` |

## Multi-TU Mode

The `--link-mode` flag supports `single_tu` (default) and `multi_tu`.

**Single-TU** (default): m2c emits one amalgamated `.c` file. The runner compiles it directly
with `cc -O0/-O2`, optionally with ASan+UBSan.

**Multi-TU**: m2c uses `--emit-per-module` to emit separate C files per module:
- `_common.h` -- runtime header + all module type/prototype/extern declarations
- `<Module>.c` -- module variable definitions, procedure bodies, init function
- `_main.c` -- the main module
- `_manifest.txt` -- list of all `.c` files to compile

The runner compiles each `.c` independently (`-c` to `.o`), then links all `.o` files into
one executable. This tests that:
- Exported symbols have correct external linkage (no stale `static`)
- `extern` declarations in `_common.h` match the actual definitions
- Module-prefixed names prevent symbol collisions across translation units
- Init functions are properly declared and called

The `mtu_linkage_trap` test specifically validates this: two modules export identically-named
symbols (`value`, `GetValue`). If the codegen emitted bare C names, multi-TU would fail at
link time with duplicate symbol errors.

## Output

All artifacts are written to `tests/adversarial/out/<timestamp>/`:
- `report.json` -- machine-readable results (includes `link_mode` and `strict` fields)
- `<category>/<test>/output.c` -- generated C files
- `<category>/<test>/exe_*` -- compiled executables
- `fuzz/failures/` -- any inputs that crashed the parser/compiler

## Adding a New Test

1. Create a directory under `programs/<category>/<test_name>/`
2. Write `.def` and `.mod` files
3. Add an entry to `tests.json`:
   ```json
   {
     "name": "my_test",
     "category": "semantics",
     "dir": "programs/semantics/my_test",
     "main": "MyTest.mod",
     "include_dirs": ["."],
     "m2plus": false,
     "expected_exit": 0,
     "expected_stdout": "expected output\n",
     "tags": ["ci"]
   }
   ```
4. Run: `python3 run_adversarial.py --category semantics`

### Test entry fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Unique test identifier |
| `category` | yes | One of the category names above |
| `dir` | yes | Directory containing source files (relative to `tests/adversarial/`) |
| `main` | yes | Entry module filename |
| `include_dirs` | yes | Include paths for `-I` (relative to `dir`, or `@root/` for repo root) |
| `m2plus` | no | `true` to pass `--m2plus` flag |
| `expected_exit` | no | Expected exit code (default: 0) |
| `expected_stdout` | no | Exact expected stdout |
| `expected_stdout_contains` | no | List of substrings that must appear in stdout |
| `expect_compile_fail` | no | `true` if transpile should fail |
| `compile_fail_match` | no | Substring that must appear in error output |
| `c_scan` | no | `true` to run post-transpile collision scan |
| `extra_c_files` | no | Additional C files to compile+link |
| `extra_cflags` | no | Additional C compiler flags |
| `extra_ldflags` | no | Additional linker flags |
| `skip_sanitizers` | no | `true` to skip ASan/UBSan for this test |
| `strict` | no | `true` = test only runs when `--strict on`; skipped otherwise |
| `tags` | no | `["ci"]` = runs in CI; `["local_only"]` = skipped in CI mode |

## CI Integration

See `ci_github_actions_snippet.yml` for a GitHub Actions example.
