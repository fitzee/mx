//! Centralized Modula-2 / Modula-2+ language documentation registry.
//!
//! All built-in documentation lives here. LSP handlers (hover, completion,
//! signature help) pull from this registry rather than embedding ad-hoc strings.

use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Keyword,
    BuiltinType,
    BuiltinProc,
    BuiltinConst,
    StdlibModule,
    Construct,
}

#[derive(Debug, Clone)]
pub struct LangDoc {
    pub name: &'static str,
    pub kind: DocKind,
    pub signature: Option<&'static str>,
    pub summary: &'static str,
    pub details: Option<&'static str>,
}

/// O(1) lookup by uppercase name.
static REGISTRY: LazyLock<HashMap<&'static str, LangDoc>> = LazyLock::new(|| {
    let mut m = HashMap::with_capacity(200);
    for doc in ALL_DOCS {
        m.insert(doc.name, doc.clone());
    }
    m
});

/// Look up language documentation by name (case-insensitive).
pub fn lookup(name: &str) -> Option<&'static LangDoc> {
    // Fast path: try exact match first (names are stored uppercase)
    if let Some(doc) = REGISTRY.get(name) {
        return Some(doc);
    }
    // Slow path: uppercase and retry
    let upper = name.to_ascii_uppercase();
    REGISTRY.get(upper.as_str())
}

/// Format a LangDoc as markdown hover content.
pub fn format_hover(doc: &LangDoc) -> String {
    let mut md = String::new();
    md.push_str("```modula2\n");
    if let Some(sig) = doc.signature {
        md.push_str(sig);
    } else {
        md.push_str(doc.name);
    }
    md.push_str("\n```\n");
    md.push_str(doc.summary);
    if let Some(details) = doc.details {
        md.push_str("\n\n");
        md.push_str(details);
    }
    md
}

// ── Registry entries ────────────────────────────────────────────────

static ALL_DOCS: &[LangDoc] = &[
    // ── Built-in types ──────────────────────────────────────────────
    LangDoc {
        name: "INTEGER",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE INTEGER"),
        summary: "Signed whole number type (32-bit).",
        details: Some("Range: MIN(INTEGER) .. MAX(INTEGER). Compatible with CARDINAL in expressions via automatic widening."),
    },
    LangDoc {
        name: "CARDINAL",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE CARDINAL"),
        summary: "Unsigned whole number type (32-bit).",
        details: Some("Range: 0 .. MAX(CARDINAL)."),
    },
    LangDoc {
        name: "REAL",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE REAL"),
        summary: "Single-precision floating-point type.",
        details: None,
    },
    LangDoc {
        name: "LONGREAL",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE LONGREAL"),
        summary: "Double-precision floating-point type.",
        details: None,
    },
    LangDoc {
        name: "BOOLEAN",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE BOOLEAN"),
        summary: "Logical type with values TRUE and FALSE.",
        details: None,
    },
    LangDoc {
        name: "CHAR",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE CHAR"),
        summary: "Character type (single byte).",
        details: Some("Ordered by ASCII value. ORD(ch) returns the ordinal; CHR(n) returns the character."),
    },
    LangDoc {
        name: "BITSET",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE BITSET"),
        summary: "Set of integers in the range 0..31.",
        details: Some("Supports INCL, EXCL, IN, +, *, - operations."),
    },
    LangDoc {
        name: "PROC",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE PROC"),
        summary: "Parameterless procedure type.",
        details: Some("A variable of type PROC can hold any procedure with no parameters and no return value."),
    },
    LangDoc {
        name: "WORD",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE WORD"),
        summary: "Untyped machine word (from SYSTEM).",
        details: Some("Compatible with any type of the same size. Used for low-level programming."),
    },
    LangDoc {
        name: "BYTE",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE BYTE"),
        summary: "Untyped single byte (from SYSTEM).",
        details: None,
    },
    LangDoc {
        name: "ADDRESS",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE ADDRESS"),
        summary: "Untyped memory address (from SYSTEM).",
        details: Some("Compatible with any pointer type. ADR(x) returns the address of x."),
    },
    LangDoc {
        name: "LONGINT",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE LONGINT"),
        summary: "Signed 64-bit integer type.",
        details: None,
    },
    LangDoc {
        name: "LONGCARD",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE LONGCARD"),
        summary: "Unsigned 64-bit cardinal type.",
        details: None,
    },

    // ── Built-in constants ──────────────────────────────────────────
    LangDoc {
        name: "TRUE",
        kind: DocKind::BuiltinConst,
        signature: Some("CONST TRUE: BOOLEAN"),
        summary: "Boolean constant true.",
        details: None,
    },
    LangDoc {
        name: "FALSE",
        kind: DocKind::BuiltinConst,
        signature: Some("CONST FALSE: BOOLEAN"),
        summary: "Boolean constant false.",
        details: None,
    },
    LangDoc {
        name: "NIL",
        kind: DocKind::BuiltinConst,
        signature: Some("CONST NIL"),
        summary: "Null pointer constant. Compatible with any pointer type.",
        details: None,
    },

    // ── Built-in procedures ─────────────────────────────────────────
    LangDoc {
        name: "NEW",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE NEW(VAR p: POINTER TO T)"),
        summary: "Allocate heap storage for a pointer variable.",
        details: Some("Allocates a block of memory for the type pointed to by p and assigns its address to p."),
    },
    LangDoc {
        name: "DISPOSE",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE DISPOSE(VAR p: POINTER TO T)"),
        summary: "Deallocate heap storage previously allocated by NEW.",
        details: Some("Releases the memory pointed to by p and sets p to NIL."),
    },
    LangDoc {
        name: "INC",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE INC(VAR x: ordinal [; n: INTEGER])"),
        summary: "Increment a variable by n (default 1).",
        details: Some("x must be an INTEGER, CARDINAL, or enumeration type. If n is omitted, increments by 1."),
    },
    LangDoc {
        name: "DEC",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE DEC(VAR x: ordinal [; n: INTEGER])"),
        summary: "Decrement a variable by n (default 1).",
        details: Some("x must be an INTEGER, CARDINAL, or enumeration type. If n is omitted, decrements by 1."),
    },
    LangDoc {
        name: "INCL",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE INCL(VAR s: BITSET; element: CARDINAL)"),
        summary: "Include an element in a set.",
        details: None,
    },
    LangDoc {
        name: "EXCL",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE EXCL(VAR s: BITSET; element: CARDINAL)"),
        summary: "Exclude an element from a set.",
        details: None,
    },
    LangDoc {
        name: "HALT",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE HALT"),
        summary: "Terminate program execution immediately.",
        details: None,
    },
    LangDoc {
        name: "ABS",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE ABS(x: numeric): numeric"),
        summary: "Return the absolute value of x.",
        details: Some("Works with INTEGER, CARDINAL, REAL, and LONGREAL."),
    },
    LangDoc {
        name: "ODD",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE ODD(x: INTEGER): BOOLEAN"),
        summary: "Return TRUE if x is odd.",
        details: None,
    },
    LangDoc {
        name: "CAP",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE CAP(ch: CHAR): CHAR"),
        summary: "Return the uppercase form of ch.",
        details: Some("If ch is a lowercase letter, returns the corresponding uppercase letter. Otherwise returns ch unchanged."),
    },
    LangDoc {
        name: "ORD",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE ORD(x: CHAR | enumeration): CARDINAL"),
        summary: "Return the ordinal number of x.",
        details: Some("For CHAR, returns the ASCII code. For enumerations, returns the position (starting at 0)."),
    },
    LangDoc {
        name: "CHR",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE CHR(n: CARDINAL): CHAR"),
        summary: "Return the character with ordinal number n.",
        details: None,
    },
    LangDoc {
        name: "VAL",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE VAL(T, x): T"),
        summary: "Convert x to type T.",
        details: Some("T must be a scalar type. The value is converted without range checking."),
    },
    LangDoc {
        name: "HIGH",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE HIGH(a: ARRAY OF T): CARDINAL"),
        summary: "Return the upper bound of an open array parameter.",
        details: Some("Only valid for open array parameters (ARRAY OF T). Returns the index of the last element."),
    },
    LangDoc {
        name: "SIZE",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE SIZE(T | x): CARDINAL"),
        summary: "Return the storage size in bytes of a type or variable.",
        details: None,
    },
    LangDoc {
        name: "TSIZE",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE TSIZE(T): CARDINAL"),
        summary: "Return the storage size in bytes of type T (from SYSTEM).",
        details: None,
    },
    LangDoc {
        name: "ADR",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE ADR(x): ADDRESS"),
        summary: "Return the memory address of variable x (from SYSTEM).",
        details: None,
    },
    LangDoc {
        name: "MAX",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE MAX(T): T"),
        summary: "Return the maximum value of a scalar type T.",
        details: Some("T may be INTEGER, CARDINAL, REAL, LONGREAL, CHAR, BOOLEAN, or an enumeration."),
    },
    LangDoc {
        name: "MIN",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE MIN(T): T"),
        summary: "Return the minimum value of a scalar type T.",
        details: None,
    },
    LangDoc {
        name: "FLOAT",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE FLOAT(x: INTEGER): REAL"),
        summary: "Convert an integer value to REAL.",
        details: None,
    },
    LangDoc {
        name: "TRUNC",
        kind: DocKind::BuiltinProc,
        signature: Some("PROCEDURE TRUNC(x: REAL): INTEGER"),
        summary: "Truncate a REAL value to INTEGER (toward zero).",
        details: None,
    },

    // ── Standard library modules ────────────────────────────────────
    LangDoc {
        name: "INOUT",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE InOut"),
        summary: "Standard text input/output.",
        details: Some("Exports: Read, ReadString, ReadInt, ReadCard, ReadChar, Write, WriteString, WriteInt, WriteCard, WriteHex, WriteOct, WriteChar, WriteLn, Done, OpenInput, OpenOutput, CloseInput, CloseOutput."),
    },
    LangDoc {
        name: "REALINOUT",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE RealInOut"),
        summary: "Real number input/output.",
        details: Some("Exports: ReadReal, WriteReal, WriteFixPt, WriteRealOct, Done."),
    },
    LangDoc {
        name: "MATHLIB0",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE MathLib0"),
        summary: "Mathematical functions.",
        details: Some("Exports: sqrt, sin, cos, arctan, exp, ln, entier, real."),
    },
    LangDoc {
        name: "STRINGS",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Strings"),
        summary: "String manipulation procedures.",
        details: Some("Exports: Assign, Concat, Length, Compare, Copy, Pos, Delete, Insert, Extract, Append."),
    },
    LangDoc {
        name: "TERMINAL",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Terminal"),
        summary: "Direct terminal I/O.",
        details: Some("Exports: Read, Write, WriteLn, WriteString, ReadChar, WriteChar."),
    },
    LangDoc {
        name: "FILESYSTEM",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE FileSystem"),
        summary: "File operations.",
        details: Some("Exports: File (type), Lookup, Close, ReadChar, WriteChar, ReadWord, WriteWord, SetPos, GetPos, Length, Rename, Delete."),
    },
    LangDoc {
        name: "STORAGE",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Storage"),
        summary: "Dynamic memory allocation.",
        details: Some("Exports: ALLOCATE, DEALLOCATE. Used implicitly by NEW and DISPOSE."),
    },
    LangDoc {
        name: "SYSTEM",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE SYSTEM"),
        summary: "Low-level system operations.",
        details: Some("Exports: WORD, BYTE, ADDRESS, ADR, TSIZE."),
    },
    LangDoc {
        name: "CONVERSIONS",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Conversions"),
        summary: "Number/string conversions.",
        details: Some("Exports: IntToStr, StrToInt, CardToStr, StrToCard, RealToStr."),
    },
    LangDoc {
        name: "ARGS",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Args"),
        summary: "Command-line argument access.",
        details: Some("Exports: ArgCount, GetArg."),
    },
    LangDoc {
        name: "STEXTIO",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE STextIO"),
        summary: "ISO-style text I/O.",
        details: Some("Exports: WriteChar, ReadChar, WriteLn, WriteString, SkipLine."),
    },
    LangDoc {
        name: "SWHOLEIO",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE SWholeIO"),
        summary: "ISO-style whole number I/O.",
        details: Some("Exports: ReadInt, WriteInt, ReadCard, WriteCard."),
    },
    LangDoc {
        name: "SREALIO",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE SRealIO"),
        summary: "ISO-style real number I/O.",
        details: Some("Exports: ReadReal, WriteReal, WriteFloat, WriteFixed."),
    },

    // ── M2+ standard library modules ────────────────────────────────
    LangDoc {
        name: "THREAD",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Thread"),
        summary: "Thread creation and management (M2+).",
        details: Some("Exports: T (type), Fork, Join. Requires pthreads."),
    },
    LangDoc {
        name: "MUTEX",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Mutex"),
        summary: "Mutual exclusion locks (M2+).",
        details: Some("Exports: T (type), Create, Lock, Unlock, Destroy."),
    },
    LangDoc {
        name: "CONDITION",
        kind: DocKind::StdlibModule,
        signature: Some("MODULE Condition"),
        summary: "Condition variables for thread synchronization (M2+).",
        details: Some("Exports: T (type), Create, Wait, Signal, Broadcast, Destroy."),
    },

    // ── Keywords: control flow ──────────────────────────────────────
    LangDoc {
        name: "IF",
        kind: DocKind::Keyword,
        signature: Some("IF expr THEN stmts {ELSIF expr THEN stmts} [ELSE stmts] END"),
        summary: "Conditional statement.",
        details: None,
    },
    LangDoc {
        name: "WHILE",
        kind: DocKind::Keyword,
        signature: Some("WHILE expr DO stmts END"),
        summary: "Pre-tested loop. Repeats while the condition is TRUE.",
        details: None,
    },
    LangDoc {
        name: "REPEAT",
        kind: DocKind::Keyword,
        signature: Some("REPEAT stmts UNTIL expr"),
        summary: "Post-tested loop. Repeats until the condition is TRUE.",
        details: Some("The loop body executes at least once."),
    },
    LangDoc {
        name: "FOR",
        kind: DocKind::Keyword,
        signature: Some("FOR ident := expr TO expr [BY const] DO stmts END"),
        summary: "Counted loop.",
        details: Some("The control variable is incremented (or decremented if BY is negative) after each iteration."),
    },
    LangDoc {
        name: "LOOP",
        kind: DocKind::Keyword,
        signature: Some("LOOP stmts END"),
        summary: "Unconditional loop. Terminated only by EXIT.",
        details: None,
    },
    LangDoc {
        name: "EXIT",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Exit the innermost enclosing LOOP.",
        details: None,
    },
    LangDoc {
        name: "CASE",
        kind: DocKind::Keyword,
        signature: Some("CASE expr OF case {| case} [ELSE stmts] END"),
        summary: "Multi-way branch on a value.",
        details: Some("Case labels must be constants. ELSE handles unmatched values."),
    },
    LangDoc {
        name: "WITH",
        kind: DocKind::Keyword,
        signature: Some("WITH designator DO stmts END"),
        summary: "Open a record scope for unqualified field access.",
        details: None,
    },
    LangDoc {
        name: "RETURN",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Return from a procedure, optionally with a value.",
        details: Some("In a function procedure, RETURN must include a value of the return type."),
    },

    // ── Keywords: declarations ──────────────────────────────────────
    LangDoc {
        name: "MODULE",
        kind: DocKind::Construct,
        signature: Some("MODULE name; [imports] [export] block name."),
        summary: "Declare a program module.",
        details: Some("A module is the top-level compilation unit. DEFINITION MODULE declares an interface; IMPLEMENTATION MODULE provides the body."),
    },
    LangDoc {
        name: "PROCEDURE",
        kind: DocKind::Construct,
        signature: Some("PROCEDURE name [(params)] [: returnType]; block name;"),
        summary: "Declare a procedure.",
        details: Some("Parameters may be value or VAR (by reference). ARRAY OF T declares an open array parameter."),
    },
    LangDoc {
        name: "RECORD",
        kind: DocKind::Construct,
        signature: Some("RECORD fieldList {; fieldList} END"),
        summary: "Declare a record type with named fields.",
        details: Some("Fields are accessed with dot notation. Records may contain variant parts (CASE tag OF)."),
    },
    LangDoc {
        name: "ARRAY",
        kind: DocKind::Construct,
        signature: Some("ARRAY indexType {, indexType} OF elementType"),
        summary: "Declare an array type.",
        details: Some("The index type must be an ordinal type or subrange. Multi-dimensional arrays use comma-separated index types."),
    },
    LangDoc {
        name: "POINTER",
        kind: DocKind::Construct,
        signature: Some("POINTER TO type"),
        summary: "Declare a pointer type.",
        details: Some("Pointer variables are allocated with NEW and deallocated with DISPOSE. Dereference with ^."),
    },
    LangDoc {
        name: "SET",
        kind: DocKind::Construct,
        signature: Some("SET OF baseType"),
        summary: "Declare a set type.",
        details: Some("The base type must be an ordinal type with at most 32 values. Supports +, *, -, /, IN, INCL, EXCL."),
    },
    LangDoc {
        name: "VAR",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Introduce variable declarations, or mark a parameter as passed by reference.",
        details: None,
    },
    LangDoc {
        name: "CONST",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Introduce constant declarations.",
        details: Some("Constants must be compile-time evaluable expressions."),
    },
    LangDoc {
        name: "TYPE",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Introduce type declarations.",
        details: None,
    },
    LangDoc {
        name: "BEGIN",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Begin the statement sequence of a block.",
        details: None,
    },
    LangDoc {
        name: "END",
        kind: DocKind::Keyword,
        signature: None,
        summary: "End a block, record, or control structure.",
        details: None,
    },
    LangDoc {
        name: "DEFINITION",
        kind: DocKind::Keyword,
        signature: Some("DEFINITION MODULE name; definitions END name."),
        summary: "Declare a definition module (interface).",
        details: Some("Defines the public interface of a module. Must have a corresponding IMPLEMENTATION MODULE."),
    },
    LangDoc {
        name: "IMPLEMENTATION",
        kind: DocKind::Keyword,
        signature: Some("IMPLEMENTATION MODULE name; block name."),
        summary: "Declare an implementation module.",
        details: Some("Provides the body for procedures declared in the corresponding DEFINITION MODULE."),
    },
    LangDoc {
        name: "FROM",
        kind: DocKind::Keyword,
        signature: Some("FROM module IMPORT ident {, ident};"),
        summary: "Import specific symbols from a module.",
        details: None,
    },
    LangDoc {
        name: "IMPORT",
        kind: DocKind::Keyword,
        signature: Some("IMPORT module {, module};"),
        summary: "Import a module for qualified access (Module.Symbol).",
        details: None,
    },
    LangDoc {
        name: "EXPORT",
        kind: DocKind::Keyword,
        signature: Some("EXPORT [QUALIFIED] ident {, ident};"),
        summary: "Export symbols from a local module.",
        details: Some("QUALIFIED export requires callers to use qualified names."),
    },

    // ── Keywords: other ─────────────────────────────────────────────
    LangDoc {
        name: "AND",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Logical AND operator (short-circuit evaluation).",
        details: None,
    },
    LangDoc {
        name: "OR",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Logical OR operator (short-circuit evaluation).",
        details: None,
    },
    LangDoc {
        name: "NOT",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Logical NOT operator.",
        details: None,
    },
    LangDoc {
        name: "DIV",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Integer division (truncates toward zero).",
        details: None,
    },
    LangDoc {
        name: "MOD",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Modulo operator (remainder of integer division).",
        details: None,
    },
    LangDoc {
        name: "IN",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Set membership test.",
        details: Some("Returns TRUE if the left operand is a member of the right operand set."),
    },
    LangDoc {
        name: "DO",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Begin the body of a WHILE, FOR, or WITH statement.",
        details: None,
    },
    LangDoc {
        name: "THEN",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Separate the condition from the body in IF or ELSIF.",
        details: None,
    },
    LangDoc {
        name: "ELSE",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Alternative branch in IF, CASE, or TYPECASE.",
        details: None,
    },
    LangDoc {
        name: "ELSIF",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Additional conditional branch in an IF statement.",
        details: None,
    },
    LangDoc {
        name: "OF",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Introduces case labels in CASE, or element type in ARRAY/SET.",
        details: None,
    },
    LangDoc {
        name: "TO",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Upper bound in a FOR loop or subrange.",
        details: None,
    },
    LangDoc {
        name: "BY",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Step value in a FOR loop.",
        details: Some("If omitted, the step defaults to 1."),
    },
    LangDoc {
        name: "UNTIL",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Termination condition for REPEAT loop.",
        details: None,
    },
    LangDoc {
        name: "QUALIFIED",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Mark exported symbols as requiring qualified access.",
        details: None,
    },

    // ── Modula-2+ keywords ──────────────────────────────────────────
    LangDoc {
        name: "TRY",
        kind: DocKind::Keyword,
        signature: Some("TRY stmts EXCEPT handler {| handler} [ELSE stmts] [FINALLY stmts] END"),
        summary: "Exception handling block (M2+).",
        details: Some("EXCEPT handlers catch named exceptions. FINALLY executes regardless of whether an exception occurred."),
    },
    LangDoc {
        name: "EXCEPT",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Begin exception handler section in a TRY block (M2+).",
        details: None,
    },
    LangDoc {
        name: "FINALLY",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Begin cleanup section in a TRY block (M2+). Always executes.",
        details: None,
    },
    LangDoc {
        name: "RAISE",
        kind: DocKind::Keyword,
        signature: Some("RAISE exceptionName"),
        summary: "Raise a named exception (M2+).",
        details: None,
    },
    LangDoc {
        name: "EXCEPTION",
        kind: DocKind::Keyword,
        signature: Some("EXCEPTION name;"),
        summary: "Declare a named exception (M2+).",
        details: None,
    },
    LangDoc {
        name: "LOCK",
        kind: DocKind::Keyword,
        signature: Some("LOCK mutex DO stmts END"),
        summary: "Acquire a mutex for the duration of the block (M2+).",
        details: Some("The mutex is released when the block exits, including on exceptions."),
    },
    LangDoc {
        name: "TYPECASE",
        kind: DocKind::Construct,
        signature: Some("TYPECASE expr OF type: stmts {| type: stmts} [ELSE stmts] END"),
        summary: "Runtime type dispatch on REFANY values (M2+).",
        details: None,
    },
    LangDoc {
        name: "REF",
        kind: DocKind::Construct,
        signature: Some("REF type"),
        summary: "Traced reference type (M2+). Heap-allocated, optionally garbage-collected.",
        details: None,
    },
    LangDoc {
        name: "REFANY",
        kind: DocKind::BuiltinType,
        signature: Some("TYPE REFANY"),
        summary: "Universal reference type (M2+). Compatible with any REF type.",
        details: Some("Use TYPECASE to dispatch on the actual type at runtime."),
    },
    LangDoc {
        name: "OBJECT",
        kind: DocKind::Construct,
        signature: Some("TYPE T = [ParentType] OBJECT fields METHODS methods END"),
        summary: "Object type with vtable-based method dispatch (M2+).",
        details: Some("Objects support single inheritance. Methods are dispatched dynamically through a vtable."),
    },
    LangDoc {
        name: "METHODS",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Begin method declarations in an OBJECT type (M2+).",
        details: None,
    },
    LangDoc {
        name: "OVERRIDES",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Begin method override declarations in an OBJECT type (M2+).",
        details: None,
    },
    LangDoc {
        name: "BRANDED",
        kind: DocKind::Keyword,
        signature: Some("BRANDED REF type"),
        summary: "Create a unique (branded) reference type (M2+).",
        details: Some("Branded references are structurally distinct even if the target type is the same."),
    },
    LangDoc {
        name: "SAFE",
        kind: DocKind::Keyword,
        signature: Some("SAFE MODULE name;"),
        summary: "Annotate a module as safe (M2+). Parsed but not enforced.",
        details: None,
    },
    LangDoc {
        name: "UNSAFE",
        kind: DocKind::Keyword,
        signature: Some("UNSAFE MODULE name;"),
        summary: "Annotate a module as unsafe (M2+). Parsed but not enforced.",
        details: None,
    },
    LangDoc {
        name: "RETRY",
        kind: DocKind::Keyword,
        signature: None,
        summary: "Re-execute the TRY block from the beginning (M2+).",
        details: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_case_insensitive() {
        assert!(lookup("INTEGER").is_some());
        assert!(lookup("integer").is_some());
        assert!(lookup("Integer").is_some());
    }

    #[test]
    fn test_lookup_builtin_proc() {
        let doc = lookup("NEW").unwrap();
        assert_eq!(doc.kind, DocKind::BuiltinProc);
        assert!(doc.signature.is_some());
        assert!(!doc.summary.is_empty());
    }

    #[test]
    fn test_lookup_keyword() {
        let doc = lookup("MODULE").unwrap();
        assert!(matches!(doc.kind, DocKind::Construct));
        assert!(doc.signature.is_some());
    }

    #[test]
    fn test_lookup_stdlib_module() {
        let doc = lookup("INOUT").unwrap();
        assert_eq!(doc.kind, DocKind::StdlibModule);
        assert!(doc.details.is_some());
    }

    #[test]
    fn test_lookup_nonexistent() {
        assert!(lookup("NOTAREAL_THING_12345").is_none());
    }

    #[test]
    fn test_format_hover_with_details() {
        let doc = lookup("NEW").unwrap();
        let md = format_hover(doc);
        assert!(md.contains("```modula2"));
        assert!(md.contains("PROCEDURE NEW"));
        assert!(md.contains("Allocate"));
    }

    #[test]
    fn test_format_hover_without_details() {
        let doc = lookup("BOOLEAN").unwrap();
        let md = format_hover(doc);
        assert!(md.contains("TYPE BOOLEAN"));
        assert!(md.contains("TRUE and FALSE"));
    }

    #[test]
    fn test_all_entries_have_nonempty_summary() {
        for doc in ALL_DOCS {
            assert!(!doc.summary.is_empty(), "empty summary for {}", doc.name);
            assert!(!doc.name.is_empty());
        }
    }

    #[test]
    fn test_registry_size() {
        // Ensure no duplicate names (HashMap would overwrite)
        assert_eq!(REGISTRY.len(), ALL_DOCS.len());
    }
}
