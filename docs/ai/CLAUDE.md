# Instructions for AI Coding Agents

You are working with the **mx** Modula-2 compiler toolchain. mx compiles PIM4 Modula-2 (and Modula-2+ extensions) by transpiling to C, then invoking the system C compiler. It ships with 32 libraries (`m2*`), a package manager (`mxpkg`), an LSP server, and a VS Code extension. This file tells you how to write correct Modula-2 code and interact with the build system.

---

## Required Reading Order

Before writing any Modula-2 code, read these files in order:

1. **LANGUAGE_RULES.md** -- hard rules the compiler enforces. Violating any of these produces errors.
2. **CHEATSHEET.md** -- syntax quick reference. Copy these patterns exactly.
3. **IDIOMS.md** -- idiomatic patterns for common tasks. Use these as templates.
4. **DEPENDENCIES.md** -- how modules are resolved, what libraries exist, and how to declare dependencies.
5. **BUILD.md** -- how m2.toml works, transitive dependencies, incremental builds, debug builds.
6. **API.md** -- procedure signatures for all 32 libraries. Use this to find the right procedure to call.

All files are in `docs/ai/`. The grammar is in `docs/lang/grammar.md`. Detailed library docs are in `docs/libs/<library>/`.

---

## Hard Invariants

These rules are **non-negotiable**. Breaking any of them produces compiler errors or broken builds.

1. **Semicolons separate, they do not terminate.** No semicolon before `END`, `ELSE`, `ELSIF`, or `UNTIL`.
2. **Not-equal is `#`.** Not `<>`, not `!=`.
3. **Procedures end with their name.** `END ProcName;` is mandatory, not `END;`.
4. **Module name must match filename.** `MODULE Foo` lives in `Foo.mod`. `DEFINITION MODULE Foo` lives in `Foo.def`.
5. **Definition modules have no procedure bodies.** No `BEGIN` block in `.def` files.
6. **No implicit type conversions.** Use `FLOAT()`, `TRUNC()`, `ORD()`, `CHR()`, `VAL()`, `INTEGER()`, `CARDINAL()`.
7. **Strings are `ARRAY [0..N] OF CHAR`.** There is no string type.
8. **Comments `(* *)` nest.** Never write `**` inside a comment -- `(**` opens a nested level.
9. **EXPORT is not used.** Everything in a `.def` file is automatically exported.
10. **Open arrays are not dynamic.** `ARRAY OF CHAR` in a parameter accepts any fixed array. You cannot resize it.

---

## Common Model Mistakes

These are errors that language models make repeatedly. Read carefully.

### Inventing modules that do not exist

Do **not** import from modules you made up. Every `FROM X IMPORT` must use a real module from either:
- The standard library (see DEPENDENCIES.md, section 2)
- A library in `libs/` (see DEPENDENCIES.md, library table)
- A module in the current project

**Bad:**
```modula-2
FROM StringUtils IMPORT Trim;      (* StringUtils does not exist *)
FROM Console IMPORT Print;          (* Console does not exist *)
FROM Memory IMPORT Alloc;           (* Memory does not exist *)
```

**Good:**
```modula-2
FROM Strings IMPORT Assign, Length;  (* standard library *)
FROM InOut IMPORT WriteString;       (* standard library *)
FROM Storage IMPORT ALLOCATE;        (* standard library *)
```

### Confusing library names with module names

The library is `m2bytes`. The modules inside it are `ByteBuf`, `Codec`, `Hex`.

**Bad:** `FROM m2bytes IMPORT ByteBuf;`
**Good:** `FROM ByteBuf IMPORT Buf, Init, Free;`

### Adding semicolons before END

```modula-2
(* BAD *)
IF x > 0 THEN
  y := 1;        (* <-- this semicolon is wrong *)
END;

(* GOOD *)
IF x > 0 THEN
  y := 1
END;
```

### Using Pascal or C syntax

```modula-2
(* BAD *)
IF x <> 0 THEN       (* Pascal not-equal *)
IF x != 0 THEN       (* C not-equal *)
name := "hello";     (* cannot assign string literal to array *)

(* GOOD *)
IF x # 0 THEN
Assign("hello", name);
```

### Missing RETURN on all paths

```modula-2
(* BAD -- no RETURN when x <= 0 *)
PROCEDURE Positive(x: INTEGER): BOOLEAN;
BEGIN
  IF x > 0 THEN RETURN TRUE END
END Positive;

(* GOOD *)
PROCEDURE Positive(x: INTEGER): BOOLEAN;
BEGIN
  RETURN x > 0
END Positive;
```

### Writing procedure bodies in .def files

```modula-2
(* BAD -- .def files declare, they do not define *)
DEFINITION MODULE Util;
PROCEDURE Max(a, b: INTEGER): INTEGER;
BEGIN
  IF a > b THEN RETURN a ELSE RETURN b END
END Max;
END Util.

(* GOOD *)
DEFINITION MODULE Util;
PROCEDURE Max(a, b: INTEGER): INTEGER;
END Util.
```

### Forgetting to null-terminate string checks

Strings in fixed arrays are null-terminated. When iterating, check for `0C`:

```modula-2
FOR i := 0 TO HIGH(s) DO
  IF s[i] = 0C THEN (* end of string *) EXIT END;
  (* process s[i] *)
END;
```

---

## Build Commands

```bash
# Compile a single file
mx compile src/Main.mod -o myapp

# Build a project (uses m2.toml)
mx build

# Run a project
mx run

# Run tests
mx test

# Clean build artifacts
mx clean

# Initialize a new project
mx init myproject
```

### Compiler Flags

| Flag | Purpose |
|------|---------|
| `-o <path>` | Output binary path |
| `-I <dir>` | Add include path |
| `-g` / `--debug` | Debug build (DWARF, #line directives) |
| `--m2plus` | Enable Modula-2+ extensions |
| `--feature <name>` | Enable feature gate |
| `--verbose` | Show compilation steps |
| `-c` | Compile to .c only (no linking) |
| `--lsp` | Start LSP server |

---

## Project Structure

A typical mx project:

```
myproject/
  m2.toml         # Project manifest
  src/
    Main.mod      # Program module (entry point)
    Utils.def     # Definition module
    Utils.mod     # Implementation module
  tests/
    Main.mod      # Test entry point
```

### Creating a Module Pair

1. Create `MyModule.def` with type and procedure declarations (no bodies).
2. Create `MyModule.mod` with `IMPLEMENTATION MODULE MyModule;` and all procedure bodies.
3. Signatures in `.mod` must exactly match `.def`.
4. Both filenames must match the module name exactly (PascalCase).

---

## Modula-2+ Extensions

Only available with `--m2plus` flag or `m2plus=true` in m2.toml.

| Feature | Syntax |
|---------|--------|
| Exceptions | `EXCEPTION E; RAISE E; TRY ... EXCEPT E DO ... FINALLY ... END` |
| REF types | `REF T`, `REFANY`, `BRANDED REF T` |
| Object types | `TYPE T = OBJECT ... METHODS ... END` |
| LOCK statement | `LOCK mutex DO ... END` |
| TYPECASE | `TYPECASE ref OF T(v) => ... END` |
| SAFE/UNSAFE | `UNSAFE MODULE M;` (parsed, not enforced) |

Do **not** use any of these in PIM4 code (the default edition).

---

## Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Module names | PascalCase | `ByteBuf`, `HashMap` |
| Procedures | PascalCase | `Init`, `AppendByte` |
| Types | PascalCase | `Buf`, `NodePtr` |
| Constants | PascalCase | `MaxSize`, `MaxKeyLen` |
| Variables | camelCase | `lineCount`, `isDir` |
| Parameters | camelCase | `initialCap`, `wrapWidth` |
| Record fields | camelCase | `data`, `len`, `cap` |

---

## When in Doubt

- Check DEPENDENCIES.md for the exact module name before writing an import.
- Check API.md for procedure signatures before calling a library procedure.
- Check CHEATSHEET.md for correct syntax of any construct.
- Check LANGUAGE_RULES.md if the compiler gives an unexpected error.
- Copy patterns from IDIOMS.md rather than inventing your own.
- Read `docs/lang/grammar.md` for the full formal grammar.
- Read `docs/libs/<library>/` for detailed library documentation.
