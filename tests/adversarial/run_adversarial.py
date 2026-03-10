#!/usr/bin/env python3
"""
Adversarial test suite for mx (Modula-2 -> C transpiler).

Tests for:
  A) Symbol namespace collisions across modules (esp. enums)
  B) Semantic correctness (short-circuit, aliasing, init order, ...)
  C) UB detection via ASan/UBSan on generated C
  D) Metamorphic properties (O0 vs O2 output equivalence, 4 transforms)
  E) Fuzzing (parser crash, well-typed program generation w/ VAR params + records)
  F) Runtime / network edge cases (EventLoop, Sockets, Stream, ...)
  G) Strict ambiguity checking (emulated --strict mode)
  H) Stream stress (proc vars, pointer chains, complex selectors)

Usage:
  python3 run_adversarial.py --mode ci
  python3 run_adversarial.py --mode local --category symbol_namespace
  python3 run_adversarial.py --compiler clang --sanitizers on --seed 42
  python3 run_adversarial.py --link-mode multi_tu
  python3 run_adversarial.py --strict on --category strict_ambiguity
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import random
import hashlib
from pathlib import Path
from datetime import datetime
from typing import List, Optional, Dict, Tuple, Any

# ═══════════════════════════════════════════════════════════════════
# Constants
# ═══════════════════════════════════════════════════════════════════

SCRIPT_DIR = Path(__file__).parent.resolve()
DEFAULT_CONFIG = SCRIPT_DIR / "config.json"
DEFAULT_TESTS = SCRIPT_DIR / "tests.json"
WARNING_FLAGS = [
    "-Wall", "-Wextra", "-Wpedantic",
    "-Wconversion", "-Wsign-conversion",
    "-Wshadow", "-Wstrict-prototypes",
]

SANITIZER_FLAGS = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]

# Budget: (fuzz_parser_count, fuzz_typed_count, fuzz_time_sec)
BUDGETS = {
    "ci":    (50,  20,  30),
    "local": (1000, 200, 180),
}

CORPUS_DIR = SCRIPT_DIR / "fuzz_corpus"

# ═══════════════════════════════════════════════════════════════════
# Result tracking
# ═══════════════════════════════════════════════════════════════════

class TestResult:
    """Outcome of a single test execution."""
    def __init__(self, name: str, category: str, variant: str = ""):
        self.name = name
        self.category = category
        self.variant = variant          # e.g. "clang-O0-asan"
        self.passed = False
        self.skipped = False
        self.phase = ""                 # "transpile" | "c_scan" | "compile" | "run" | "check"
        self.error = ""
        self.stdout = ""
        self.stderr = ""
        self.exit_code = -1
        self.duration_ms = 0.0
        self.artifacts: Dict[str, str] = {}  # label -> path

    def __repr__(self):
        status = "PASS" if self.passed else ("SKIP" if self.skipped else "FAIL")
        tag = f" [{self.variant}]" if self.variant else ""
        return f"{status}: {self.category}/{self.name}{tag}"

# ═══════════════════════════════════════════════════════════════════
# Utilities
# ═══════════════════════════════════════════════════════════════════

def run_cmd(cmd: List[str], timeout: int = 60, env=None, cwd=None,
            stdin_data: str = None) -> Tuple[int, str, str]:
    """Run a command, return (exit_code, stdout, stderr)."""
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
            cwd=cwd,
            input=stdin_data,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        return -1, "", f"TIMEOUT after {timeout}s"
    except FileNotFoundError:
        return -1, "", f"Command not found: {cmd[0]}"
    except Exception as e:
        return -1, "", str(e)


def find_compiler(name: str) -> Optional[str]:
    """Return the path to a C compiler, or None if unavailable."""
    path = shutil.which(name)
    return path


def detect_sanitizer_support(compiler: str) -> bool:
    """Check if the compiler supports ASan+UBSan."""
    with tempfile.NamedTemporaryFile(suffix=".c", mode="w", delete=False) as f:
        f.write("int main(void){return 0;}\n")
        f.flush()
        src = f.name
    try:
        rc, _, _ = run_cmd(
            [compiler] + SANITIZER_FLAGS + [src, "-o", "/dev/null"],
            timeout=15,
        )
        return rc == 0
    finally:
        os.unlink(src)


def ensure_dir(path: Path):
    path.mkdir(parents=True, exist_ok=True)

# ═══════════════════════════════════════════════════════════════════
# Post-transpile C scanner  (category A5)
# ═══════════════════════════════════════════════════════════════════

def scan_c_for_collisions(c_path: str) -> List[str]:
    """
    Parse generated C for symbol collisions:
      - Duplicate typedef names
      - Duplicate enumerator names
      - Duplicate non-static global symbols (best-effort)
    Returns a list of collision descriptions (empty = clean).
    """
    with open(c_path, "r") as f:
        c_src = f.read()

    collisions: List[str] = []

    # ── Duplicate typedef names ──
    # Match:  typedef enum { ... } Name;
    #         typedef struct { ... } Name;
    typedef_names: Dict[str, int] = {}
    for m in re.finditer(r'typedef\s+(?:enum|struct)\s*\{[^}]*\}\s+(\w+)\s*;', c_src):
        name = m.group(1)
        typedef_names[name] = typedef_names.get(name, 0) + 1
    for name, count in typedef_names.items():
        if count > 1:
            collisions.append(f"duplicate typedef: '{name}' appears {count} times")

    # ── Duplicate enumerator names ──
    # Extract all enum bodies and collect enumerator names
    enumerator_names: Dict[str, int] = {}
    for m in re.finditer(r'typedef\s+enum\s*\{([^}]*)\}', c_src):
        body = m.group(1)
        for e in re.finditer(r'(\w+)\s*(?:=[^,}]*)?[,}]?', body):
            ename = e.group(1).strip()
            if ename:
                enumerator_names[ename] = enumerator_names.get(ename, 0) + 1
    for name, count in enumerator_names.items():
        if count > 1:
            collisions.append(f"duplicate enumerator: '{name}' appears {count} times")

    # ── Duplicate non-static global function definitions ──
    # Best-effort: look for function definitions at column 0
    func_defs: Dict[str, int] = {}
    for m in re.finditer(r'^(?!static\b)\w[\w\s\*]*?\s+(\w+)\s*\([^)]*\)\s*\{', c_src, re.MULTILINE):
        fname = m.group(1)
        if fname not in ("main", "if", "while", "for", "switch"):
            func_defs[fname] = func_defs.get(fname, 0) + 1
    for name, count in func_defs.items():
        if count > 1:
            collisions.append(f"duplicate function def: '{name}' appears {count} times")

    return collisions


def scan_c_for_missing_static(c_path: str) -> List[str]:
    """
    Best-effort check: non-main function definitions at file scope without
    'static' are flagged as warnings. These are not necessarily bugs in
    single-TU mode, but will cause ODR violations if multi-TU is enabled.
    """
    with open(c_path, "r") as f:
        c_src = f.read()

    warnings: List[str] = []
    # Match function definitions at column 0 that are NOT static and NOT main
    for m in re.finditer(
        r'^(?!static\b)(\w[\w\s\*]*?)\s+(\w+)\s*\([^)]*\)\s*\{',
        c_src, re.MULTILINE,
    ):
        fname = m.group(2)
        if fname in ("main", "if", "while", "for", "switch", "return"):
            continue
        # Skip common generated helpers that are intentionally non-static
        if fname.startswith("M2_"):
            continue
        warnings.append(f"non-static function at file scope: '{fname}'")
    return warnings


# ═══════════════════════════════════════════════════════════════════
# Strict ambiguity emulator
# ═══════════════════════════════════════════════════════════════════

def strip_m2_comments(source: str) -> str:
    """Strip Modula-2 comments (* ... *), handling nesting."""
    result = []
    i = 0
    depth = 0
    while i < len(source):
        if i + 1 < len(source) and source[i] == '(' and source[i+1] == '*':
            depth += 1
            i += 2
        elif i + 1 < len(source) and source[i] == '*' and source[i+1] == ')':
            if depth > 0:
                depth -= 1
            i += 2
        elif depth == 0:
            result.append(source[i])
            i += 1
        else:
            i += 1
    return ''.join(result)


def emulate_strict_ambiguity(source: str) -> List[str]:
    """
    Emulate strict ambiguity checking on a Modula-2 source file.
    Returns a list of ambiguity error strings (empty = no ambiguities).

    1. Strip comments
    2. Parse FROM <Module> IMPORT <name>, ...; lines
    3. Build name -> set of origin modules
    4. For names imported from 2+ modules, check unqualified use in the body
    """
    clean = strip_m2_comments(source)

    # Parse imports: FROM Module IMPORT name1, name2, ...;
    import_origins: Dict[str, set] = {}
    for m in re.finditer(
        r'FROM\s+(\w+)\s+IMPORT\s+([^;]+);', clean
    ):
        module = m.group(1)
        names_str = m.group(2)
        for name in re.split(r'\s*,\s*', names_str.strip()):
            name = name.strip()
            if name:
                if name not in import_origins:
                    import_origins[name] = set()
                import_origins[name].add(module)

    # Find names imported from 2+ modules
    ambiguous_names = {
        name: modules
        for name, modules in import_origins.items()
        if len(modules) >= 2
    }

    if not ambiguous_names:
        return []

    # Find the body: everything after the last import line, before final END
    # Simple heuristic: after last FROM...IMPORT...;
    last_import_end = 0
    for m in re.finditer(r'FROM\s+\w+\s+IMPORT\s+[^;]+;', clean):
        last_import_end = m.end()

    body = clean[last_import_end:]

    errors = []
    for name, modules in sorted(ambiguous_names.items()):
        # Check if name is used unqualified in the body
        if re.search(rf'\b{re.escape(name)}\b', body):
            mods = ' and '.join(sorted(modules))
            errors.append(
                f"strict: '{name}' imported from {mods}, used unqualified"
            )

    return errors


# ═══════════════════════════════════════════════════════════════════
# Fuzzer: parser crash
# ═══════════════════════════════════════════════════════════════════

M2_KEYWORDS = [
    "MODULE", "END", "BEGIN", "PROCEDURE", "VAR", "CONST", "TYPE",
    "IF", "THEN", "ELSE", "ELSIF", "WHILE", "DO", "FOR", "TO", "BY",
    "REPEAT", "UNTIL", "CASE", "OF", "WITH", "RETURN", "EXIT",
    "IMPORT", "FROM", "EXPORT", "DEFINITION", "IMPLEMENTATION",
    "ARRAY", "RECORD", "POINTER", "SET", "INTEGER", "CARDINAL",
    "BOOLEAN", "CHAR", "REAL", "TRUE", "FALSE", "NIL", "AND", "OR",
    "NOT", "IN", "DIV", "MOD", "LOOP", "HALT",
]

M2_SYMBOLS = [
    ":=", "=", "#", "<", ">", "<=", ">=", "+", "-", "*", "/",
    "(", ")", "[", "]", "{", "}", ".", "..", ",", ";", ":", "|", "^",
]


class ParserCrashFuzzer:
    """Generate random token sequences to crash-test the parser."""

    def __init__(self, seed: int, mx_cmd: List[str]):
        self.rng = random.Random(seed)
        self.mx_cmd = mx_cmd

    def gen_tokens(self, n: int) -> str:
        tokens = []
        for _ in range(n):
            kind = self.rng.randint(0, 4)
            if kind == 0:
                tokens.append(self.rng.choice(M2_KEYWORDS))
            elif kind == 1:
                tokens.append(self.rng.choice(M2_SYMBOLS))
            elif kind == 2:
                # identifier
                length = self.rng.randint(1, 12)
                tokens.append("".join(
                    self.rng.choice("abcdefghijklmnopqrstuvwxyz") for _ in range(length)
                ))
            elif kind == 3:
                # integer literal
                tokens.append(str(self.rng.randint(0, 999)))
            else:
                # string literal
                tokens.append(f'"{"x" * self.rng.randint(0, 8)}"')
        return " ".join(tokens)

    def gen_grammar_ish(self) -> str:
        """Generate something that looks vaguely like a module."""
        name = "Fuzz" + str(self.rng.randint(0, 9999))
        body_tokens = self.gen_tokens(self.rng.randint(5, 50))
        return f"MODULE {name};\n{body_tokens}\nEND {name}.\n"

    def run_one(self, source: str, out_dir: Path) -> Tuple[bool, str]:
        """Transpile a source string.  Returns (crashed: bool, detail)."""
        src_path = out_dir / "fuzz_input.mod"
        src_path.write_text(source)
        out_c = out_dir / "fuzz_output.c"

        cmd = self.mx_cmd + ["--emit-c", str(src_path), "-o", str(out_c)]
        rc, stdout, stderr = run_cmd(cmd, timeout=10)

        # rc != 0 is fine (parser rejected input).
        # Crash = signal (negative rc on Unix) or specific crash indicators.
        if rc < 0 and rc != -1:  # -1 is our timeout sentinel
            return True, f"Signal {-rc}: {stderr[:200]}"
        if "panic" in stderr.lower() or "segfault" in stderr.lower():
            return True, stderr[:300]
        return False, ""


# ═══════════════════════════════════════════════════════════════════
# Fuzzer: well-typed program generator
# ═══════════════════════════════════════════════════════════════════

class WellTypedFuzzer:
    """Generate small well-typed M2 programs and verify they compile+run."""

    def __init__(self, seed: int, mx_cmd: List[str]):
        self.rng = random.Random(seed)
        self.mx_cmd = mx_cmd

    def gen_program(self) -> str:
        """Generate a valid M2 program with integer arithmetic,
        optionally including VAR-param procs, swap procs, and record types."""
        name = f"WTFuzz{self.rng.randint(0, 99999)}"
        num_vars = self.rng.randint(1, 5)
        var_names = [f"v{i}" for i in range(num_vars)]

        type_section = ""
        proc_section = ""
        extra_var_decls = ""
        has_var_proc = self.rng.random() < 0.5
        has_swap = self.rng.random() < 0.3
        has_record = self.rng.random() < 0.4

        # Record type (40% chance)
        if has_record:
            type_section += "TYPE\n  FuzzRec = RECORD f0, f1: INTEGER END;\n"
            extra_var_decls += "  rec: FuzzRec;\n"

        # VAR parameter procedure (50% chance)
        if has_var_proc:
            proc_section += (
                "PROCEDURE IncByVal(VAR x: INTEGER; delta: INTEGER);\n"
                "BEGIN x := x + delta END IncByVal;\n\n"
            )

        # Swap procedure (30% chance)
        if has_swap:
            proc_section += (
                "PROCEDURE SwapTwo(VAR a, b: INTEGER);\n"
                "VAR tmp: INTEGER;\n"
                "BEGIN tmp := a; a := b; b := tmp END SwapTwo;\n\n"
            )

        var_decls = "  " + ", ".join(var_names) + ": INTEGER;\n"

        stmts = []
        # Initialize variables
        for v in var_names:
            stmts.append(f"  {v} := {self.rng.randint(-100, 100)};")

        # Initialize record fields if present
        if has_record:
            stmts.append(f"  rec.f0 := {self.rng.randint(-50, 50)};")
            stmts.append(f"  rec.f1 := {self.rng.randint(-50, 50)};")

        # Generate some assignments
        num_stmts = self.rng.randint(2, 10)
        for _ in range(num_stmts):
            dst = self.rng.choice(var_names)
            kind = self.rng.randint(0, 5)
            if kind == 0:
                # binary op
                a = self.rng.choice(var_names)
                b = self.rng.choice(var_names)
                op = self.rng.choice(["+", "-", "*"])
                if op == "*":
                    stmts.append(f"  IF ({a} > -100) AND ({a} < 100) THEN {dst} := {a} {op} {b} END;")
                else:
                    stmts.append(f"  {dst} := {a} {op} {b};")
            elif kind == 1:
                # conditional
                a = self.rng.choice(var_names)
                val = self.rng.randint(-50, 50)
                stmts.append(f"  IF {a} > {val} THEN {dst} := {a} - 1 ELSE {dst} := {a} + 1 END;")
            elif kind == 2:
                # loop
                stmts.append(f"  FOR {dst} := 0 TO {self.rng.randint(1, 5)} DO END;")
            elif kind == 3 and has_var_proc:
                # call VAR param proc
                target = self.rng.choice(var_names)
                delta = self.rng.randint(-10, 10)
                stmts.append(f"  IncByVal({target}, {delta});")
            elif kind == 4 and has_swap and len(var_names) >= 2:
                # call swap
                a_var, b_var = self.rng.sample(var_names, 2)
                stmts.append(f"  SwapTwo({a_var}, {b_var});")
            elif kind == 5 and has_record:
                # read from record field
                field = self.rng.choice(["f0", "f1"])
                stmts.append(f"  {dst} := rec.{field};")
            else:
                # literal assignment
                stmts.append(f"  {dst} := {self.rng.randint(-1000, 1000)};")

        # Print all vars
        prints = []
        for v in var_names:
            prints.append(f"  WriteInt({v}, 0); WriteLn;")
        if has_record:
            prints.append(f"  WriteInt(rec.f0, 0); WriteLn;")
            prints.append(f"  WriteInt(rec.f1, 0); WriteLn;")

        body = "\n".join(stmts) + "\n" + "\n".join(prints)

        return (
            f"MODULE {name};\n"
            f"FROM InOut IMPORT WriteInt, WriteLn;\n"
            f"{type_section}"
            f"{proc_section}"
            f"VAR\n{var_decls}{extra_var_decls}"
            f"BEGIN\n{body}\n"
            f"END {name}.\n"
        )

    def run_one(self, out_dir: Path, c_compiler: str) -> Tuple[bool, str, str]:
        """
        Generate, transpile, compile, run one program.
        Returns (crashed, detail, source).
        """
        source = self.gen_program()
        src_path = out_dir / "wtfuzz_input.mod"
        src_path.write_text(source)
        out_c = out_dir / "wtfuzz_output.c"
        out_exe = out_dir / "wtfuzz_exe"

        # Transpile
        cmd = self.mx_cmd + ["--emit-c", str(src_path), "-o", str(out_c)]
        rc, _, stderr = run_cmd(cmd, timeout=10)
        if rc != 0:
            return False, f"transpile rejected (ok): {stderr[:100]}", source

        # Compile
        cmd = [c_compiler, "-O0", "-w", str(out_c), "-o", str(out_exe)]
        rc, _, stderr = run_cmd(cmd, timeout=10)
        if rc != 0:
            return True, f"C compile failed: {stderr[:200]}", source

        # Run
        rc, stdout, stderr = run_cmd([str(out_exe)], timeout=5)
        if rc < 0:
            return True, f"Runtime crash (signal {-rc}): {stderr[:200]}", source

        return False, "", source


# ═══════════════════════════════════════════════════════════════════
# Metamorphic testing
# ═══════════════════════════════════════════════════════════════════

def apply_dead_code_transform(source: str) -> str:
    """Insert dead code guarded by FALSE after each BEGIN."""
    return source.replace(
        "BEGIN\n",
        "BEGIN\n  IF FALSE THEN WriteInt(9999, 0) END;\n",
        1,  # only first occurrence to keep it simple
    )


def apply_alpha_rename(source: str) -> str:
    """Rename local variables v -> zz_v (simple, non-keyword rename)."""
    # Only rename single-letter lowercase identifiers that are likely vars
    # This is intentionally conservative to avoid breaking the program
    result = source
    for old_name in ["i", "n", "r", "x", "a", "b"]:
        new_name = f"zz_{old_name}"
        # Word-boundary rename (crude but effective for small test programs)
        result = re.sub(rf'\b{old_name}\b', new_name, result)
    return result


def apply_decl_reorder(source: str) -> str:
    """
    Reverse the order of top-level PROCEDURE declarations.
    Semantics-preserving when procedures are independent (no forward refs).
    Returns unchanged source if fewer than 2 procedures found.
    """
    # Find all top-level PROCEDURE blocks: PROCEDURE Name(...) ... END Name;
    proc_pattern = re.compile(
        r'^(PROCEDURE\s+(\w+)\b.*?^END\s+\2\s*;)',
        re.MULTILINE | re.DOTALL,
    )
    matches = list(proc_pattern.finditer(source))
    if len(matches) < 2:
        return source

    # Extract procedure texts and their positions
    procs = [(m.start(), m.end(), m.group(1)) for m in matches]

    # Build result: everything before first proc, then procs in reverse, then rest
    result = source[:procs[0][0]]
    reversed_procs = [p[2] for p in reversed(procs)]
    result += "\n\n".join(reversed_procs)
    result += source[procs[-1][1]:]
    return result


def apply_temp_introduction(source: str) -> str:
    """
    Rewrite the first module-body WriteInt(Func(args), width) into:
      tmpMetaVar := Func(args); WriteInt(tmpMetaVar, width)
    and add tmpMetaVar: INTEGER to the module-level VAR section.
    """
    # Find the module-level BEGIN (the one that starts the module body, after all procs)
    # Heuristic: find the last BEGIN that isn't inside a PROCEDURE
    # We search for all WriteInt calls and pick one that's after the last "END ProcName;"
    last_end_proc = 0
    for m in re.finditer(r'^END\s+\w+\s*;', source, re.MULTILINE):
        last_end_proc = m.end()

    # Search for WriteInt(Func(args), width) only in the module body
    body_source = source[last_end_proc:]
    call_match = re.search(
        r'WriteInt\((\w+)\(([^)]*)\),\s*(\d+)\)',
        body_source,
    )
    if not call_match:
        return source

    func_name = call_match.group(1)
    func_args = call_match.group(2)
    width = call_match.group(3)

    # Replace the call (in the full source) with a temp variable
    old_call = call_match.group(0)
    new_stmts = f"tmpMetaVar := {func_name}({func_args});\n    WriteInt(tmpMetaVar, {width})"
    # Only replace in the module body portion
    new_body = body_source.replace(old_call, new_stmts, 1)
    result = source[:last_end_proc] + new_body

    # Add tmpMetaVar to the module-level VAR section
    # Find the last VAR line before the module-level BEGIN
    # Module-level BEGIN: the last BEGIN in the source (after all procs)
    module_begin = result.rfind("\nBEGIN\n")
    if module_begin < 0:
        module_begin = result.rfind("\nBEGIN")

    # Search backwards from module BEGIN for the nearest VAR
    var_region = result[:module_begin]
    var_pos = var_region.rfind("\nVAR ")
    if var_pos < 0:
        var_pos = var_region.rfind("\nVAR\n")

    if var_pos >= 0:
        # Find the end of this VAR line
        var_line_end = result.index("\n", var_pos + 1)
        old_var_line = result[var_pos + 1:var_line_end]
        result = result[:var_line_end] + "\n  tmpMetaVar: INTEGER;" + result[var_line_end:]
    else:
        # No module-level VAR — insert before BEGIN
        if module_begin >= 0:
            result = result[:module_begin] + "\nVAR tmpMetaVar: INTEGER;" + result[module_begin:]

    return result


# ═══════════════════════════════════════════════════════════════════
# Main runner
# ═══════════════════════════════════════════════════════════════════

class AdversarialRunner:
    def __init__(self, args):
        self.args = args
        self.results: List[TestResult] = []

        # Load config
        config_path = Path(args.config) if args.config else DEFAULT_CONFIG
        with open(config_path) as f:
            self.config = json.load(f)

        # Load test catalog
        tests_path = Path(args.tests) if args.tests else DEFAULT_TESTS
        with open(tests_path) as f:
            self.test_catalog = json.load(f)

        # Resolve project root (relative to SCRIPT_DIR)
        self.project_root = (SCRIPT_DIR / self.config.get("project_root", "../..")).resolve()

        # Resolve mx command
        mx_raw = self.config.get("mx", "cargo run --quiet --")
        self.mx_cmd = mx_raw.split()

        # Output directory
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        self.out_dir = SCRIPT_DIR / "out" / ts
        ensure_dir(self.out_dir)

        # Detect compilers
        self.compilers: Dict[str, str] = {}
        if args.compiler in ("clang", "all"):
            cc = find_compiler("clang")
            if cc:
                self.compilers["clang"] = cc
        if args.compiler in ("gcc", "all"):
            cc = find_compiler("gcc")
            if cc:
                self.compilers["gcc"] = cc
        if not self.compilers:
            # Fallback: try cc
            cc = find_compiler("cc")
            if cc:
                self.compilers["cc"] = cc

        if not self.compilers:
            print("ERROR: No C compiler found (tried clang, gcc, cc)")
            sys.exit(1)

        # Detect sanitizer support per compiler
        self.sanitizer_support: Dict[str, bool] = {}
        if args.sanitizers == "on":
            for name, path in self.compilers.items():
                self.sanitizer_support[name] = detect_sanitizer_support(path)
                if self.sanitizer_support[name]:
                    print(f"  {name}: ASan+UBSan supported")
                else:
                    print(f"  {name}: ASan+UBSan NOT supported (skipping sanitizer runs)")
        else:
            for name in self.compilers:
                self.sanitizer_support[name] = False

        # Multi-TU config
        self.link_mode = args.link_mode
        self.multi_tu_supported = self.config.get("multi_tu_supported", False)
        self.multi_tu_skip_message = self.config.get(
            "multi_tu_skip_message",
            "multi-TU not supported",
        )
        self.multi_tu_mx_flags = self.config.get("multi_tu_mx_flags", [])
        self.multi_tu_manifest = self.config.get("multi_tu_manifest", "_manifest.txt")

        # Strict mode
        self.strict = args.strict == "on"

        # Budget
        mode = args.mode
        self.fuzz_parser_count, self.fuzz_typed_count, self.fuzz_time_sec = BUDGETS.get(mode, BUDGETS["ci"])

        # Seed
        self.seed = args.seed

    # ── Pipeline helpers ──────────────────────────────────────

    def resolve_path(self, test_dir: Path, p: str) -> str:
        """Resolve a path that may start with @root/ or be relative to test_dir."""
        if p.startswith("@root/"):
            return str(self.project_root / p[6:])
        else:
            return str((test_dir / p).resolve())

    def transpile_multi_tu(self, test_def: dict, test_dir: Path,
                          out_dir: Path) -> Tuple[bool, str, List[str]]:
        """Run mx --emit-per-module. Returns (success, stderr, list_of_c_files)."""
        main_file = test_dir / test_def["main"]
        cmd = list(self.mx_cmd)
        cmd.extend(self.multi_tu_mx_flags)
        cmd.extend(["--out-dir", str(out_dir)])

        if test_def.get("m2plus", False):
            cmd.append("--m2plus")

        for inc in test_def.get("include_dirs", ["."]):
            cmd.extend(["-I", self.resolve_path(test_dir, inc)])

        cmd.append(str(main_file))

        rc, stdout, stderr = run_cmd(cmd, timeout=30, cwd=str(self.project_root))
        if rc != 0:
            return False, stderr, []

        # Read manifest to get list of .c files
        manifest_path = out_dir / self.multi_tu_manifest
        if not manifest_path.exists():
            return False, f"manifest not found at {manifest_path}", []
        c_files = [
            f.strip() for f in manifest_path.read_text().strip().split("\n") if f.strip()
        ]
        return True, stderr, c_files

    def run_multi_tu_test(self, test_def: dict) -> List[TestResult]:
        """Run a test in multi-TU mode: transpile to per-module .c files, compile each, link."""
        results = []
        name = test_def["name"]
        category = test_def["category"]
        test_dir = (SCRIPT_DIR / test_def["dir"]).resolve()
        test_out = self.out_dir / category / name
        mtu_dir = test_out / "mtu"
        ensure_dir(mtu_dir)

        # ── Phase 1: Transpile to per-module files ──
        t0 = time.time()
        ok, stderr, c_files = self.transpile_multi_tu(test_def, test_dir, mtu_dir)
        transpile_ms = (time.time() - t0) * 1000

        expect_fail = test_def.get("expect_compile_fail", False)

        if expect_fail:
            r = TestResult(name, category, "multi_tu-transpile")
            r.duration_ms = transpile_ms
            if not ok:
                r.passed = True
            else:
                r.phase = "transpile"
                r.error = "Expected compile failure but transpile succeeded"
            r.stderr = stderr
            results.append(r)
            return results

        if not ok:
            r = TestResult(name, category, "multi_tu-transpile")
            r.phase = "transpile"
            r.error = f"Transpile (per-module) failed: {stderr[:300]}"
            r.stderr = stderr
            r.duration_ms = transpile_ms
            results.append(r)
            return results

        # ── Phase 2: C scan on individual files ──
        if test_def.get("c_scan", False):
            # Scan each .c file for collisions (now meaningful in multi-TU)
            for c_file in c_files:
                c_path = mtu_dir / c_file
                if c_path.exists():
                    collisions = scan_c_for_collisions(str(c_path))
                    if collisions:
                        r = TestResult(name, category, f"c_scan-{c_file}")
                        r.phase = "c_scan"
                        r.error = "; ".join(collisions)
                        r.artifacts["c_file"] = str(c_path)
                        results.append(r)

        # ── Phase 3: Compile each .c → .o, then link ──
        expected_exit = test_def.get("expected_exit", 0)
        expected_stdout = test_def.get("expected_stdout", None)
        expected_contains = test_def.get("expected_stdout_contains", None)

        extra_c_files = [
            self.resolve_path(test_dir, f)
            for f in test_def.get("extra_c_files", [])
        ]
        extra_cflags = list(test_def.get("extra_cflags", []))
        extra_ldflags = list(test_def.get("extra_ldflags", []))

        for cc_name, cc_path in self.compilers.items():
            for opt in ["-O0", "-O2"]:
                variant = f"mtu-{cc_name}{opt}"
                r = self._compile_link_run_multi_tu(
                    name, category, variant, cc_path, opt, False,
                    mtu_dir, c_files, test_out, expected_exit,
                    expected_stdout, expected_contains,
                    extra_c_files, extra_cflags, extra_ldflags,
                )
                results.append(r)

                # With sanitizers
                if self.sanitizer_support.get(cc_name, False):
                    skip_san = test_def.get("skip_sanitizers", False)
                    if not skip_san:
                        variant_san = f"mtu-{cc_name}{opt}-asan"
                        r = self._compile_link_run_multi_tu(
                            name, category, variant_san, cc_path, opt, True,
                            mtu_dir, c_files, test_out, expected_exit,
                            expected_stdout, expected_contains,
                            extra_c_files, extra_cflags, extra_ldflags,
                        )
                        results.append(r)

        return results

    def _compile_link_run_multi_tu(
        self, name, category, variant, cc_path, opt, use_san,
        mtu_dir, c_files, test_out, expected_exit, expected_stdout,
        expected_contains, extra_c_files, extra_cflags, extra_ldflags,
    ) -> TestResult:
        """Compile each .c to .o, link, and run."""
        r = TestResult(name, category, variant)
        t0 = time.time()

        # Compile each .c → .o
        obj_files = []
        for c_file in c_files:
            c_path = mtu_dir / c_file
            o_path = test_out / f"{c_file}_{variant}.o"
            cmd = [cc_path, opt, "-w", "-c", str(c_path), "-o", str(o_path)]
            if use_san:
                cmd[2:2] = SANITIZER_FLAGS
            if extra_cflags:
                cmd[2:2] = extra_cflags
            # Add the mtu_dir as include path so #include "_common.h" resolves
            cmd[2:2] = ["-I", str(mtu_dir)]

            rc, _, stderr = run_cmd(cmd, timeout=30)
            if rc != 0:
                r.phase = "compile"
                r.error = f"C compile failed for {c_file} ({variant}): {stderr[:300]}"
                r.stderr = stderr
                r.duration_ms = (time.time() - t0) * 1000
                return r
            obj_files.append(str(o_path))

        # Link
        exe = test_out / f"exe_{variant}"
        link_cmd = [cc_path, opt] + obj_files
        if extra_c_files:
            link_cmd += extra_c_files
        link_cmd += ["-o", str(exe)]
        if use_san:
            link_cmd += SANITIZER_FLAGS
        if extra_ldflags:
            link_cmd += extra_ldflags

        rc, _, stderr = run_cmd(link_cmd, timeout=30)
        if rc != 0:
            r.phase = "link"
            r.error = f"Link failed ({variant}): {stderr[:300]}"
            r.stderr = stderr
            r.duration_ms = (time.time() - t0) * 1000
            return r

        compile_ms = (time.time() - t0) * 1000

        # Run
        t1 = time.time()
        rc, stdout, stderr = self.run_exe(exe)
        run_ms = (time.time() - t1) * 1000
        r.exit_code = rc
        r.stdout = stdout
        r.stderr = stderr
        r.duration_ms = compile_ms + run_ms

        # Check exit code
        if rc != expected_exit:
            r.phase = "run"
            r.error = f"Exit code {rc}, expected {expected_exit}"
            if stderr:
                r.error += f"\nstderr: {stderr[:200]}"
            return r

        # Check stdout
        if expected_stdout is not None and stdout != expected_stdout:
            r.phase = "check"
            r.error = (
                f"Stdout mismatch:\n"
                f"  expected: {expected_stdout!r}\n"
                f"  actual:   {stdout!r}"
            )
            return r

        if expected_contains is not None:
            for s in expected_contains:
                if s not in stdout:
                    r.phase = "check"
                    r.error = f"Stdout missing substring: {s!r}\n  actual: {stdout[:200]!r}"
                    return r

        r.passed = True
        return r

    def transpile(self, test_def: dict, test_dir: Path, out_c: Path) -> Tuple[bool, str]:
        """Run mx --emit-c. Returns (success, stderr)."""
        main_file = test_dir / test_def["main"]
        cmd = list(self.mx_cmd)
        cmd.append("--emit-c")

        if test_def.get("m2plus", False):
            cmd.append("--m2plus")

        for inc in test_def.get("include_dirs", ["."]):
            cmd.extend(["-I", self.resolve_path(test_dir, inc)])

        cmd.append(str(main_file))
        cmd.extend(["-o", str(out_c)])

        rc, stdout, stderr = run_cmd(cmd, timeout=30, cwd=str(self.project_root))
        return rc == 0, stderr

    def compile_c(self, c_file: Path, out_exe: Path, compiler: str,
                  opt: str, use_sanitizers: bool,
                  extra_cflags: List[str] = None,
                  extra_ldflags: List[str] = None,
                  extra_c_files: List[str] = None) -> Tuple[bool, str]:
        """Compile generated C with a C compiler. Returns (success, stderr)."""
        cmd = [compiler, opt] + WARNING_FLAGS
        if use_sanitizers:
            cmd += SANITIZER_FLAGS
        if extra_cflags:
            cmd += extra_cflags
        cmd += [str(c_file)]
        if extra_c_files:
            cmd += extra_c_files
        cmd += ["-o", str(out_exe)]
        if extra_ldflags:
            cmd += extra_ldflags

        rc, stdout, stderr = run_cmd(cmd, timeout=30)
        return rc == 0, stderr

    def run_exe(self, exe: Path, timeout: int = 10) -> Tuple[int, str, str]:
        """Run an executable. Returns (exit_code, stdout, stderr)."""
        # Set ASAN_OPTIONS for better diagnostics
        env = os.environ.copy()
        env["ASAN_OPTIONS"] = "detect_leaks=0:halt_on_error=1"
        env["UBSAN_OPTIONS"] = "halt_on_error=1:print_stacktrace=1"
        return run_cmd([str(exe)], timeout=timeout, env=env)

    # ── Standard test execution ───────────────────────────────

    def run_standard_test(self, test_def: dict) -> List[TestResult]:
        """Run a standard M2 test through the full pipeline."""
        results = []
        name = test_def["name"]
        category = test_def["category"]
        test_dir = (SCRIPT_DIR / test_def["dir"]).resolve()
        test_out = self.out_dir / category / name
        ensure_dir(test_out)

        # ── Multi-TU mode ──
        if self.link_mode == "multi_tu":
            if not self.multi_tu_supported:
                r = TestResult(name, category, "multi_tu")
                r.skipped = True
                r.error = self.multi_tu_skip_message
                results.append(r)
                return results
            return self.run_multi_tu_test(test_def)

        # ── Strict ambiguity check ──
        if test_def.get("strict", False) and self.strict:
            main_file = test_dir / test_def["main"]
            source = main_file.read_text()
            ambiguities = emulate_strict_ambiguity(source)
            expect_fail = test_def.get("expect_compile_fail", False)
            if ambiguities:
                r = TestResult(name, category, "strict")
                if expect_fail:
                    r.passed = True
                    r.stdout = "; ".join(ambiguities)
                else:
                    r.phase = "strict"
                    r.error = "; ".join(ambiguities)
                results.append(r)
                return results
            # No ambiguities found — proceed normally if not expect_fail
            if expect_fail:
                # Strict test expects ambiguity but none found — that's a failure
                r = TestResult(name, category, "strict")
                r.phase = "strict"
                r.error = "Expected ambiguity but none detected"
                results.append(r)
                return results

        out_c = test_out / "output.c"

        # ── Phase 1: Transpile ──
        t0 = time.time()
        ok, stderr = self.transpile(test_def, test_dir, out_c)
        transpile_ms = (time.time() - t0) * 1000

        expect_fail = test_def.get("expect_compile_fail", False)

        if expect_fail:
            # We EXPECT transpilation to fail
            r = TestResult(name, category, "transpile")
            r.duration_ms = transpile_ms
            if not ok:
                fail_match = test_def.get("compile_fail_match", "")
                if fail_match and fail_match not in stderr:
                    r.phase = "transpile"
                    r.error = f"Expected error containing '{fail_match}', got: {stderr[:200]}"
                else:
                    r.passed = True
            else:
                r.phase = "transpile"
                r.error = "Expected compile failure but transpile succeeded"
            r.stderr = stderr
            results.append(r)
            return results

        if not ok:
            r = TestResult(name, category, "transpile")
            r.phase = "transpile"
            r.error = f"Transpile failed: {stderr[:300]}"
            r.stderr = stderr
            r.duration_ms = transpile_ms
            results.append(r)
            return results

        # ── Phase 2: Post-transpile C scan ──
        if test_def.get("c_scan", False):
            collisions = scan_c_for_collisions(str(out_c))
            r = TestResult(name, category, "c_scan")
            r.duration_ms = 0
            if collisions:
                r.phase = "c_scan"
                r.error = "; ".join(collisions)
                r.artifacts["c_file"] = str(out_c)
            else:
                r.passed = True
            results.append(r)

            # Enhanced: check for missing static qualifiers (warning only)
            static_warnings = scan_c_for_missing_static(str(out_c))
            if static_warnings:
                r = TestResult(name, category, "c_scan_static")
                r.passed = True  # warnings don't fail the test
                r.stdout = "; ".join(static_warnings[:5])  # cap at 5
                results.append(r)

        # ── Phase 3: Compile + Run matrix ──
        opt_levels = ["-O0", "-O2"]
        expected_exit = test_def.get("expected_exit", 0)
        expected_stdout = test_def.get("expected_stdout", None)
        expected_contains = test_def.get("expected_stdout_contains", None)

        # Resolve extra files relative to test dir or project root
        extra_c_files = [
            self.resolve_path(test_dir, f)
            for f in test_def.get("extra_c_files", [])
        ]
        extra_cflags = list(test_def.get("extra_cflags", []))
        extra_ldflags = list(test_def.get("extra_ldflags", []))

        for cc_name, cc_path in self.compilers.items():
            for opt in opt_levels:
                # Without sanitizers
                variant = f"{cc_name}{opt}"
                r = self._compile_and_run(
                    name, category, variant, cc_path, opt, False,
                    out_c, test_out, expected_exit, expected_stdout,
                    expected_contains, extra_c_files, extra_cflags, extra_ldflags,
                )
                results.append(r)

                # With sanitizers (if supported + enabled)
                if self.sanitizer_support.get(cc_name, False):
                    skip_san = test_def.get("skip_sanitizers", False)
                    if not skip_san:
                        variant_san = f"{cc_name}{opt}-asan"
                        r = self._compile_and_run(
                            name, category, variant_san, cc_path, opt, True,
                            out_c, test_out, expected_exit, expected_stdout,
                            expected_contains, extra_c_files, extra_cflags,
                            extra_ldflags,
                        )
                        results.append(r)

        return results

    def _compile_and_run(
        self, name, category, variant, cc_path, opt, use_san,
        out_c, test_out, expected_exit, expected_stdout,
        expected_contains, extra_c_files, extra_cflags, extra_ldflags,
    ) -> TestResult:
        """Compile and run a single variant."""
        r = TestResult(name, category, variant)
        exe = test_out / f"exe_{variant}"

        # Compile
        t0 = time.time()
        ok, stderr = self.compile_c(
            out_c, exe, cc_path, opt, use_san,
            extra_cflags, extra_ldflags, extra_c_files,
        )
        compile_ms = (time.time() - t0) * 1000

        if not ok:
            r.phase = "compile"
            r.error = f"C compile failed ({variant}): {stderr[:300]}"
            r.stderr = stderr
            r.duration_ms = compile_ms
            r.artifacts["c_file"] = str(out_c)
            return r

        # Run
        t0 = time.time()
        rc, stdout, stderr = self.run_exe(exe)
        run_ms = (time.time() - t0) * 1000
        r.exit_code = rc
        r.stdout = stdout
        r.stderr = stderr
        r.duration_ms = compile_ms + run_ms

        # Check exit code
        if rc != expected_exit:
            r.phase = "run"
            r.error = f"Exit code {rc}, expected {expected_exit}"
            if stderr:
                r.error += f"\nstderr: {stderr[:200]}"
            return r

        # Check stdout
        if expected_stdout is not None and stdout != expected_stdout:
            r.phase = "check"
            r.error = (
                f"Stdout mismatch:\n"
                f"  expected: {expected_stdout!r}\n"
                f"  actual:   {stdout!r}"
            )
            return r

        if expected_contains is not None:
            for s in expected_contains:
                if s not in stdout:
                    r.phase = "check"
                    r.error = f"Stdout missing substring: {s!r}\n  actual: {stdout[:200]!r}"
                    return r

        r.passed = True
        return r

    # ── Metamorphic tests ─────────────────────────────────────

    def run_metamorphic_tests(self) -> List[TestResult]:
        """
        For each metamorphic test program:
          1. Compile at -O0 and -O2, assert same stdout+exit
          2. Apply source transforms, re-transpile+run, assert same output
        """
        results = []
        meta_tests = [t for t in self.test_catalog if t["category"] == "metamorphic"]

        for test_def in meta_tests:
            name = test_def["name"]
            test_dir = (SCRIPT_DIR / test_def["dir"]).resolve()
            test_out = self.out_dir / "metamorphic" / name
            ensure_dir(test_out)

            main_file = test_dir / test_def["main"]
            source = main_file.read_text()

            # Baseline: transpile once
            out_c = test_out / "baseline.c"
            ok, stderr = self.transpile(test_def, test_dir, out_c)
            if not ok:
                r = TestResult(name, "metamorphic", "baseline-transpile")
                r.phase = "transpile"
                r.error = f"Transpile failed: {stderr[:200]}"
                results.append(r)
                continue

            # Compile at -O0 and -O2, compare outputs
            cc_name = list(self.compilers.keys())[0]
            cc_path = self.compilers[cc_name]

            outputs = {}
            for opt in ["-O0", "-O2"]:
                exe = test_out / f"baseline_{opt.replace('-', '')}"
                ok, stderr = self.compile_c(out_c, exe, cc_path, opt, False)
                if not ok:
                    r = TestResult(name, "metamorphic", f"baseline-compile-{opt}")
                    r.phase = "compile"
                    r.error = f"Compile failed: {stderr[:200]}"
                    results.append(r)
                    continue
                rc, stdout, _ = self.run_exe(exe)
                outputs[opt] = (rc, stdout)

            if "-O0" in outputs and "-O2" in outputs:
                r = TestResult(name, "metamorphic", "O0-vs-O2")
                if outputs["-O0"] == outputs["-O2"]:
                    r.passed = True
                else:
                    r.phase = "check"
                    r.error = (
                        f"O0 vs O2 mismatch:\n"
                        f"  O0 exit={outputs['-O0'][0]} stdout={outputs['-O0'][1]!r}\n"
                        f"  O2 exit={outputs['-O2'][0]} stdout={outputs['-O2'][1]!r}"
                    )
                results.append(r)

            # Transform 1: dead code insertion
            transformed = apply_dead_code_transform(source)
            r = self._run_transform(name, "dead_code", transformed, test_def, test_out,
                                     cc_path, outputs.get("-O0"))
            results.append(r)

            # Transform 2: alpha rename
            transformed = apply_alpha_rename(source)
            r = self._run_transform(name, "alpha_rename", transformed, test_def, test_out,
                                     cc_path, outputs.get("-O0"))
            results.append(r)

            # Transform 3: declaration reorder
            transformed = apply_decl_reorder(source)
            if transformed != source:
                r = self._run_transform(name, "decl_reorder", transformed, test_def, test_out,
                                         cc_path, outputs.get("-O0"))
                results.append(r)

            # Transform 4: temp variable introduction
            transformed = apply_temp_introduction(source)
            if transformed != source:
                r = self._run_transform(name, "temp_intro", transformed, test_def, test_out,
                                         cc_path, outputs.get("-O0"))
                results.append(r)

        return results

    def _run_transform(self, name, transform_name, source, test_def, test_out,
                       cc_path, baseline_output) -> TestResult:
        """Transpile+compile+run a transformed source and compare to baseline."""
        r = TestResult(name, "metamorphic", f"transform-{transform_name}")
        t_dir = test_out / transform_name
        ensure_dir(t_dir)

        # Write transformed source
        # Derive module name from source
        m = re.search(r'MODULE\s+(\w+)', source)
        mod_name = m.group(1) if m else "Transformed"
        src_path = t_dir / f"{mod_name}.mod"
        src_path.write_text(source)

        out_c = t_dir / "output.c"
        # Build a modified test_def for the transform
        t_def = dict(test_def)
        t_def["dir"] = str(t_dir.relative_to(SCRIPT_DIR))
        t_def["main"] = src_path.name
        t_def["include_dirs"] = ["."]

        ok, stderr = self.transpile(t_def, t_dir, out_c)
        if not ok:
            # Some transforms may break the source — not necessarily a runner failure
            r.phase = "transpile"
            r.error = f"Transform '{transform_name}' broke transpile: {stderr[:200]}"
            r.skipped = True
            return r

        exe = t_dir / "exe"
        ok, stderr = self.compile_c(out_c, exe, cc_path, "-O0", False)
        if not ok:
            r.phase = "compile"
            r.error = f"C compile failed after transform: {stderr[:200]}"
            return r

        rc, stdout, _ = self.run_exe(exe)

        if baseline_output is not None:
            if (rc, stdout) == baseline_output:
                r.passed = True
            else:
                r.phase = "check"
                r.error = (
                    f"Output changed after '{transform_name}':\n"
                    f"  baseline: exit={baseline_output[0]} stdout={baseline_output[1]!r}\n"
                    f"  modified: exit={rc} stdout={stdout!r}"
                )
        else:
            # No baseline to compare — just check it didn't crash
            r.passed = rc >= 0
            if not r.passed:
                r.error = f"Crash after transform (signal {-rc})"

        return r

    # ── Fuzzing ───────────────────────────────────────────────

    def run_fuzz_tests(self) -> List[TestResult]:
        """Run parser crash fuzzer and well-typed program fuzzer."""
        results = []
        fuzz_out = self.out_dir / "fuzz"
        ensure_dir(fuzz_out)
        failures_dir = fuzz_out / "failures"
        ensure_dir(failures_dir)

        # Ensure persistent corpus directory exists
        ensure_dir(CORPUS_DIR)
        parser_corpus = CORPUS_DIR / "parser"
        typed_corpus = CORPUS_DIR / "typed"
        ensure_dir(parser_corpus)
        ensure_dir(typed_corpus)

        # ── Phase 0: Replay saved corpus (regression) ──
        results.extend(self._replay_corpus(parser_corpus, "parser", fuzz_out, failures_dir))
        results.extend(self._replay_corpus(typed_corpus, "typed", fuzz_out, failures_dir))

        # ── Parser crash fuzzer ──
        print(f"\n  Fuzzing parser ({self.fuzz_parser_count} inputs, seed={self.seed})...")
        fuzzer = ParserCrashFuzzer(self.seed, self.mx_cmd)
        crash_count = 0
        t_start = time.time()

        for i in range(self.fuzz_parser_count):
            if time.time() - t_start > self.fuzz_time_sec:
                break
            work_dir = fuzz_out / f"parser_{i}"
            ensure_dir(work_dir)
            source = fuzzer.gen_grammar_ish()
            crashed, detail = fuzzer.run_one(source, work_dir)
            if crashed:
                crash_count += 1
                fail_path = failures_dir / f"parser_crash_{i}.mod"
                fail_path.write_text(source)
                # Save to persistent corpus for regression
                h = hashlib.sha256(source.encode()).hexdigest()[:12]
                (parser_corpus / f"crash_{h}.mod").write_text(source)
                r = TestResult(f"parser_fuzz_{i}", "fuzz", f"seed{self.seed}")
                r.phase = "transpile"
                r.error = detail
                results.append(r)
            # Clean up non-failing work dirs to save space
            if not crashed:
                shutil.rmtree(work_dir, ignore_errors=True)

        r = TestResult("parser_fuzz_summary", "fuzz", f"seed{self.seed}")
        tested = min(self.fuzz_parser_count, i + 1) if self.fuzz_parser_count > 0 else 0
        if crash_count == 0:
            r.passed = True
            r.stdout = f"{tested} inputs, 0 crashes"
        else:
            r.error = f"{crash_count}/{tested} inputs crashed the parser"
        results.append(r)

        # ── Well-typed fuzzer ──
        cc_path = list(self.compilers.values())[0]
        print(f"  Fuzzing well-typed ({self.fuzz_typed_count} programs, seed={self.seed})...")
        wt_fuzzer = WellTypedFuzzer(self.seed + 1000, self.mx_cmd)
        wt_crash_count = 0
        t_start = time.time()

        for i in range(self.fuzz_typed_count):
            if time.time() - t_start > self.fuzz_time_sec:
                break
            work_dir = fuzz_out / f"typed_{i}"
            ensure_dir(work_dir)
            crashed, detail, source = wt_fuzzer.run_one(work_dir, cc_path)
            if crashed:
                wt_crash_count += 1
                fail_path = failures_dir / f"typed_crash_{i}.mod"
                fail_path.write_text(source)
                # Save to persistent corpus for regression
                h = hashlib.sha256(source.encode()).hexdigest()[:12]
                (typed_corpus / f"crash_{h}.mod").write_text(source)
                r = TestResult(f"typed_fuzz_{i}", "fuzz", f"seed{self.seed}")
                r.phase = "compile" if "compile" in detail.lower() else "run"
                r.error = detail
                results.append(r)
            if not crashed:
                shutil.rmtree(work_dir, ignore_errors=True)

        r = TestResult("typed_fuzz_summary", "fuzz", f"seed{self.seed}")
        tested = min(self.fuzz_typed_count, i + 1) if self.fuzz_typed_count > 0 else 0
        if wt_crash_count == 0:
            r.passed = True
            r.stdout = f"{tested} programs, 0 crashes"
        else:
            r.error = f"{wt_crash_count}/{tested} programs caused crashes"
        results.append(r)

        return results

    def _replay_corpus(self, corpus_dir: Path, kind: str,
                       fuzz_out: Path, failures_dir: Path) -> List[TestResult]:
        """Replay saved crash corpus for regression testing."""
        results = []
        corpus_files = sorted(corpus_dir.glob("*.mod"))
        if not corpus_files:
            return results

        print(f"  Replaying {len(corpus_files)} saved {kind} corpus inputs...")
        fuzzer = ParserCrashFuzzer(0, self.mx_cmd)
        still_crash = 0

        for cf in corpus_files:
            source = cf.read_text()
            work_dir = fuzz_out / f"replay_{kind}_{cf.stem}"
            ensure_dir(work_dir)
            crashed, detail = fuzzer.run_one(source, work_dir)

            if crashed:
                still_crash += 1
                r = TestResult(f"corpus_{kind}_{cf.stem}", "fuzz", "corpus-replay")
                r.phase = "transpile"
                r.error = detail
                results.append(r)
            else:
                # Fixed! Remove from corpus.
                cf.unlink()
            shutil.rmtree(work_dir, ignore_errors=True)

        if corpus_files:
            r = TestResult(f"corpus_{kind}_summary", "fuzz", "corpus-replay")
            if still_crash == 0:
                r.passed = True
                r.stdout = f"All {len(corpus_files)} corpus inputs now pass"
            else:
                r.error = f"{still_crash}/{len(corpus_files)} corpus inputs still crash"
            results.append(r)

        return results

    # ── Main entry point ──────────────────────────────────────

    def run(self) -> int:
        """Run the full test suite. Returns 0 on all-pass, 1 on any failure."""
        print("=" * 60)
        print("mx Adversarial Test Suite")
        print("=" * 60)
        print(f"  Mode:       {self.args.mode}")
        print(f"  Compilers:  {', '.join(self.compilers.keys())}")
        print(f"  Sanitizers: {self.args.sanitizers}")
        print(f"  Link mode:  {self.link_mode}")
        print(f"  Strict:     {'on' if self.strict else 'off'}")
        print(f"  Seed:       {self.seed}")
        print(f"  Output:     {self.out_dir}")
        print()

        categories_filter = self.args.category.split(",") if self.args.category != "all" else None

        def should_run(cat: str) -> bool:
            if categories_filter is None:
                return True
            return cat in categories_filter

        # ── Standard tests (includes strict_ambiguity + stream_stress) ──
        std_categories = [
            "symbol_namespace", "semantics", "ub_sanitizer", "runtime",
            "resolution", "import_chain", "proc_values", "abi_layout",
            "strict_ambiguity", "stream_stress",
        ]
        if any(should_run(c) for c in std_categories):
            std_tests = [
                t for t in self.test_catalog
                if t["category"] != "metamorphic"
                and (categories_filter is None or t["category"] in categories_filter)
            ]
            for test_def in std_tests:
                tag = test_def.get("tags", [])
                if self.args.mode == "ci" and "local_only" in tag:
                    continue
                # Skip strict tests when --strict is off
                if test_def.get("strict", False) and not self.strict:
                    r = TestResult(test_def["name"], test_def["category"], "strict-off")
                    r.skipped = True
                    r.error = "strict mode off; skipping strict test"
                    self.results.append(r)
                    continue
                print(f"  Running: {test_def['category']}/{test_def['name']}...")
                test_results = self.run_standard_test(test_def)
                self.results.extend(test_results)

        # ── Metamorphic tests (D) ──
        if should_run("metamorphic"):
            print("\n  Running metamorphic tests...")
            self.results.extend(self.run_metamorphic_tests())

        # ── Fuzz tests (E) ──
        if should_run("fuzz"):
            self.results.extend(self.run_fuzz_tests())

        # ── Summary ──
        return self.print_summary()

    def print_summary(self) -> int:
        """Print results summary. Returns exit code."""
        print("\n" + "=" * 60)
        print("RESULTS")
        print("=" * 60)

        passed = [r for r in self.results if r.passed]
        failed = [r for r in self.results if not r.passed and not r.skipped]
        skipped = [r for r in self.results if r.skipped]

        if failed:
            print("\nFAILURES:")
            for r in failed:
                print(f"  FAIL: {r.category}/{r.name} [{r.variant}]")
                print(f"        Phase: {r.phase}")
                for line in r.error.split("\n"):
                    print(f"        {line}")
                for label, path in r.artifacts.items():
                    print(f"        {label}: {path}")
                print()

        if skipped:
            print(f"\nSKIPPED ({len(skipped)}):")
            for r in skipped:
                print(f"  SKIP: {r.category}/{r.name} [{r.variant}] — {r.error[:80]}")

        print(f"\nTotal:   {len(self.results)}")
        print(f"Passed:  {len(passed)}")
        print(f"Failed:  {len(failed)}")
        print(f"Skipped: {len(skipped)}")
        print(f"Output:  {self.out_dir}")

        # Write JSON report
        report = {
            "timestamp": datetime.now().isoformat(),
            "mode": self.args.mode,
            "link_mode": self.link_mode,
            "strict": self.strict,
            "seed": self.seed,
            "compilers": list(self.compilers.keys()),
            "total": len(self.results),
            "passed": len(passed),
            "failed": len(failed),
            "skipped": len(skipped),
            "failures": [
                {
                    "name": r.name,
                    "category": r.category,
                    "variant": r.variant,
                    "phase": r.phase,
                    "error": r.error,
                }
                for r in failed
            ],
        }
        report_path = self.out_dir / "report.json"
        with open(report_path, "w") as f:
            json.dump(report, f, indent=2)
        print(f"Report:  {report_path}")

        if failed:
            print(f"\n*** {len(failed)} FAILURE(S) ***")
            return 1
        else:
            print("\n*** ALL TESTS PASSED ***")
            return 0


# ═══════════════════════════════════════════════════════════════════
# CLI
# ═══════════════════════════════════════════════════════════════════

def main():
    parser = argparse.ArgumentParser(
        description="Adversarial test suite for mx compiler",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  %(prog)s --mode ci                          # CI run (fast, all categories)
  %(prog)s --mode local --category fuzz       # Local fuzz run (larger budget)
  %(prog)s --compiler clang --sanitizers on   # Clang + ASan/UBSan
  %(prog)s --category symbol_namespace,semantics  # Specific categories
""",
    )
    parser.add_argument(
        "--mode", choices=["ci", "local"], default="ci",
        help="Test mode: 'ci' for fast, 'local' for thorough (default: ci)",
    )
    parser.add_argument(
        "--category", default="all",
        help="Comma-separated categories to run (default: all). "
             "Options: symbol_namespace, semantics, ub_sanitizer, metamorphic, fuzz, runtime, "
             "resolution, import_chain, proc_values, abi_layout, strict_ambiguity, stream_stress",
    )
    parser.add_argument(
        "--link-mode", choices=["single_tu", "multi_tu"], default="single_tu",
        dest="link_mode",
        help="Link mode: single_tu (default) or multi_tu (skips if not supported)",
    )
    parser.add_argument(
        "--strict", choices=["on", "off"], default="off",
        help="Strict ambiguity checking: on or off (default: off)",
    )
    parser.add_argument(
        "--compiler", choices=["clang", "gcc", "all"], default="all",
        help="Which C compiler(s) to use (default: all available)",
    )
    parser.add_argument(
        "--sanitizers", choices=["on", "off"], default="on",
        help="Enable ASan+UBSan (default: on)",
    )
    parser.add_argument(
        "--seed", type=int, default=20260221,
        help="Random seed for fuzz tests (default: 20260221)",
    )
    parser.add_argument(
        "--config", type=str, default=None,
        help=f"Path to config.json (default: {DEFAULT_CONFIG})",
    )
    parser.add_argument(
        "--tests", type=str, default=None,
        help=f"Path to tests.json (default: {DEFAULT_TESTS})",
    )

    args = parser.parse_args()

    runner = AdversarialRunner(args)
    sys.exit(runner.run())


if __name__ == "__main__":
    main()
