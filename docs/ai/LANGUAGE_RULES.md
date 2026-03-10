# mx Language Rules

Practical rules that supplement the grammar in `docs/lang/grammar.md`. Each rule addresses a pattern that AI models frequently get wrong.

---

## Strings Are Fixed Arrays

**Rule:** Modula-2 has no string type. Strings are `ARRAY [0..N] OF CHAR`. There is no heap-allocated resizable string. String literals are `ARRAY OF CHAR` constants.

Correct:
```modula-2
VAR name: ARRAY [0..63] OF CHAR;
FROM Strings IMPORT Assign;
Assign("hello", name);
```

Incorrect:
```modula-2
VAR name: STRING;          (* no STRING type *)
name := "hello";           (* cannot assign literal to array variable *)
VAR name: String.T;        (* no String module *)
```

---

## No Semicolons Before END, ELSE, ELSIF, UNTIL

**Rule:** Semicolons separate statements, they do not terminate them. A semicolon before `END`, `ELSE`, `ELSIF`, or `UNTIL` inserts an empty statement. While often harmless, it is not idiomatic. Never place a semicolon after the last statement before these keywords.

Correct:
```modula-2
IF x > 0 THEN
  y := 1
ELSE
  y := 0
END;
```

Incorrect:
```modula-2
IF x > 0 THEN
  y := 1;       (* unnecessary semicolon before ELSE *)
ELSE
  y := 0;       (* unnecessary semicolon before END *)
END;
```

---

## Module Name Must Match Filename

**Rule:** `MODULE Foo` must be in `Foo.mod`. `DEFINITION MODULE Foo` must be in `Foo.def`. `IMPLEMENTATION MODULE Foo` must be in `Foo.mod`. The closing identifier must match: `END Foo.`

Correct:
```
File: Stack.def
  DEFINITION MODULE Stack; ... END Stack.

File: Stack.mod
  IMPLEMENTATION MODULE Stack; ... END Stack.
```

Incorrect:
```
File: stack.def              (* wrong case *)
File: StackModule.def        (* name mismatch *)
  DEFINITION MODULE Stack;   (* module name != filename *)
```

---

## Definition Module Contains No Bodies

**Rule:** A `.def` file declares types, constants, and procedure signatures only. Procedure bodies go in the `.mod` file. The `.def` file has no `BEGIN` block.

Correct:
```modula-2
DEFINITION MODULE Util;
PROCEDURE Max(a, b: INTEGER): INTEGER;
END Util.
```

Incorrect:
```modula-2
DEFINITION MODULE Util;
PROCEDURE Max(a, b: INTEGER): INTEGER;
BEGIN                        (* no BEGIN in .def *)
  IF a > b THEN RETURN a ELSE RETURN b END
END Max;
END Util.
```

---

## Procedures End With Their Name

**Rule:** Every procedure body ends with `END ProcName;`. The name after `END` is mandatory and must match the procedure name.

Correct:
```modula-2
PROCEDURE Compute(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 2
END Compute;
```

Incorrect:
```modula-2
PROCEDURE Compute(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 2
END;                         (* missing procedure name *)
```

---

## Not-Equal Is # Not <>

**Rule:** The not-equal operator is `#`. Modula-2 does not use `<>` or `!=`.

Correct:
```modula-2
IF x # 0 THEN ...
```

Incorrect:
```modula-2
IF x <> 0 THEN ...          (* Pascal syntax, not valid *)
IF x != 0 THEN ...          (* C syntax, not valid *)
```

---

## No RETURN Without Value in Function Procedures

**Rule:** A procedure with a return type must return a value on every path. A procedure without a return type must not return a value.

Correct:
```modula-2
PROCEDURE IsZero(x: INTEGER): BOOLEAN;
BEGIN
  RETURN x = 0
END IsZero;

PROCEDURE Reset(VAR x: INTEGER);
BEGIN
  x := 0
END Reset;
```

Incorrect:
```modula-2
PROCEDURE IsZero(x: INTEGER): BOOLEAN;
BEGIN
  IF x = 0 THEN RETURN TRUE END
  (* missing RETURN on else path *)
END IsZero;
```

---

## VAR Parameters Are Pass-By-Reference

**Rule:** `VAR` in a parameter list means the caller's variable is modified. Without `VAR`, parameters are pass-by-value. Use `VAR` for output parameters and for large records to avoid copying.

Correct:
```modula-2
PROCEDURE Swap(VAR a, b: INTEGER);
VAR tmp: INTEGER;
BEGIN
  tmp := a; a := b; b := tmp
END Swap;
```

Incorrect:
```modula-2
PROCEDURE Swap(a, b: INTEGER);    (* no VAR -- changes lost *)
```

---

## ARRAY OF CHAR Is Open Array, Not Dynamic

**Rule:** `ARRAY OF CHAR` in a parameter is an open array -- it accepts any fixed-size `ARRAY [0..N] OF CHAR`. It is not a dynamic/resizable string. Use `HIGH(param)` to get the last valid index.

Correct:
```modula-2
PROCEDURE PrintLen(s: ARRAY OF CHAR);
BEGIN
  WriteInt(HIGH(s) + 1, 1); WriteLn
END PrintLen;
```

Incorrect:
```modula-2
PROCEDURE Grow(VAR s: ARRAY OF CHAR);
BEGIN
  (* cannot resize an open array -- size is fixed at call site *)
END Grow;
```

---

## Comments Nest

**Rule:** Modula-2 comments `(* *)` nest. An inner `(*` opens a new level that requires its own `*)`. Never write `**` inside a comment -- it opens a nested comment.

Correct:
```modula-2
(* outer (* inner *) still outer *)
(* double-star segment: matches recursively *)
```

Incorrect:
```modula-2
(* ** matches recursively *)    (* opens nested comment, never closed *)
```

---

## No Implicit Type Conversion

**Rule:** Modula-2 has no implicit widening or narrowing. Use explicit conversion functions: `FLOAT()`, `TRUNC()`, `ORD()`, `CHR()`, `VAL()`, `INTEGER()`, `CARDINAL()`.

Correct:
```modula-2
VAR i: INTEGER; r: REAL;
r := FLOAT(i);
i := TRUNC(r);
```

Incorrect:
```modula-2
r := i;        (* no implicit conversion *)
i := r;        (* no implicit conversion *)
```

---

## EXPORT Is Not Used

**Rule:** In PIM4 as implemented by mx, everything declared in a `.def` file is exported. Do not write `EXPORT` or `EXPORT QUALIFIED` -- the compiler accepts but ignores them. The `.def` file itself defines the public interface.

Correct:
```modula-2
DEFINITION MODULE Util;
PROCEDURE Max(a, b: INTEGER): INTEGER;
END Util.
```

Incorrect:
```modula-2
DEFINITION MODULE Util;
EXPORT QUALIFIED Max;        (* unnecessary, ignored *)
PROCEDURE Max(a, b: INTEGER): INTEGER;
END Util.
```

---

## Naming Conventions

**Rule:** Follow the project conventions consistently.

| Element | Convention | Example |
|---------|-----------|---------|
| Module names | PascalCase | `ByteBuf`, `HashMap`, `CLI` |
| Procedure names | PascalCase | `Init`, `AppendByte`, `ReadU16LE` |
| Type names | PascalCase | `Buf`, `Reader`, `NodePtr` |
| Constants | PascalCase or ALL_CAPS | `MaxSize`, `MaxKeyLen` |
| Variables | camelCase | `lineCount`, `isDir`, `prevX` |
| Parameters | camelCase | `initialCap`, `wrapWidth` |
| Record fields | camelCase | `data`, `len`, `cap`, `occupied` |

---

## Error Handling (PIM4)

**Rule:** Use a `VAR ok: BOOLEAN` output parameter for fallible operations. Check immediately after the call.

Correct:
```modula-2
PROCEDURE ReadByte(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;

(* caller: *)
val := ReadByte(r, ok);
IF NOT ok THEN
  (* handle error *)
END;
```

Incorrect:
```modula-2
PROCEDURE ReadByte(VAR r: Reader): CARDINAL;
(* RAISES ReadError *)          (* PIM4 has no exceptions *)
```

---

## Error Handling (M2+ only)

**Rule:** M2+ code (edition=m2plus) may use TRY/EXCEPT/FINALLY with declared exceptions. Never use exception syntax in PIM4 code.

```modula-2
EXCEPTION ParseError;

PROCEDURE Parse(input: ARRAY OF CHAR);
BEGIN
  IF input[0] = 0C THEN RAISE ParseError END;
  (* ... *)
END Parse;

(* caller: *)
TRY
  Parse(data)
EXCEPT ParseError DO
  WriteString("parse failed"); WriteLn
FINALLY
  Cleanup
END;
```

---

## Foreign C Modules

**Rule:** C functions are imported via `DEFINITION MODULE FOR "C"`. The procedures use bare C names without module prefix.

```modula-2
DEFINITION MODULE FOR "C" GfxBridge;
FROM SYSTEM IMPORT ADDRESS;
PROCEDURE gfx_init(): INTEGER;
PROCEDURE gfx_quit;
END GfxBridge.
```
