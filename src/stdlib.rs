use crate::errors::SourceLoc;
use crate::symtab::*;
use crate::types::*;

/// Register standard library module symbols into a scope
pub fn register_module(
    symtab: &mut SymbolTable,
    types: &mut TypeRegistry,
    scope: usize,
    module: &str,
) {
    let upper = module.to_ascii_uppercase();
    match upper.as_str() {
        "INOUT" => register_inout(symtab, types, scope),
        "REALINOUT" => register_realinout(symtab, types, scope),
        "MATHLIB0" | "MATHLIB" => register_mathlib(symtab, types, scope),
        "STRINGS" => register_strings(symtab, types, scope),
        "STORAGE" => register_storage(symtab, types, scope),
        "SYSTEM" => register_system(symtab, types, scope),
        "TERMINAL" => register_terminal(symtab, types, scope),
        "FILESYSTEM" => register_filesystem(symtab, types, scope),
        // ISO standard I/O modules
        "STEXTIO" => register_stextio(symtab, types, scope),
        "SWHOLEIO" => register_swholeio(symtab, types, scope),
        "SREALIO" => register_srealio(symtab, types, scope),
        "SLONGIO" => register_slongio(symtab, types, scope),
        "SIORESULT" => register_sioresult(symtab, types, scope),
        "ARGS" => register_args(symtab, types, scope),
        "BINARYIO" => register_binaryio(symtab, types, scope),
        // Modula-2+ concurrency modules
        "THREAD" => register_thread(symtab, types, scope),
        "MUTEX" => register_mutex(symtab, types, scope),
        "CONDITION" => register_condition(symtab, types, scope),
        _ => {} // Unknown module - will be resolved later from .def files
    }
}

fn def_proc(
    symtab: &mut SymbolTable,
    scope: usize,
    name: &str,
    params: Vec<ParamInfo>,
    ret: Option<TypeId>,
) {
    def_proc_doc(symtab, scope, name, params, ret, None);
}

fn def_proc_doc(
    symtab: &mut SymbolTable,
    scope: usize,
    name: &str,
    params: Vec<ParamInfo>,
    ret: Option<TypeId>,
    doc: Option<&str>,
) {
    let _ = symtab.define(
        scope,
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Procedure {
                params,
                return_type: ret,
                is_builtin: false,
            },
            typ: TY_VOID,
            exported: true,
            module: None,
            loc: SourceLoc::default(),
            doc: doc.map(|s| s.to_string()),
        },
    );
}

fn def_var(symtab: &mut SymbolTable, scope: usize, name: &str, typ: TypeId) {
    def_var_doc(symtab, scope, name, typ, None);
}

fn def_var_doc(symtab: &mut SymbolTable, scope: usize, name: &str, typ: TypeId, doc: Option<&str>) {
    let _ = symtab.define(
        scope,
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Variable,
            typ,
            exported: true,
            module: None,
            loc: SourceLoc::default(),
            doc: doc.map(|s| s.to_string()),
        },
    );
}

fn p(name: &str, typ: TypeId, is_var: bool) -> ParamInfo {
    ParamInfo {
        name: name.to_string(),
        typ,
        is_var,
    }
}

fn register_inout(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "Read", vec![p("ch", TY_CHAR, true)], None,
        Some("Read a single character from standard input."));
    def_proc_doc(symtab, scope, "ReadString", vec![p("s", TY_STRING, true)], None,
        Some("Read a whitespace-delimited string from standard input."));
    def_proc_doc(symtab, scope, "ReadInt", vec![p("n", TY_INTEGER, true)], None,
        Some("Read an INTEGER value from standard input. Sets `Done` to TRUE on success."));
    def_proc_doc(symtab, scope, "ReadCard", vec![p("n", TY_CARDINAL, true)], None,
        Some("Read a CARDINAL value from standard input. Sets `Done` to TRUE on success."));
    def_proc_doc(symtab, scope, "Write", vec![p("ch", TY_CHAR, false)], None,
        Some("Write a single character to standard output."));
    def_proc_doc(symtab, scope, "WriteString", vec![p("s", TY_STRING, false)], None,
        Some("Write a string to standard output."));
    def_proc_doc(symtab, scope, "WriteInt", vec![p("n", TY_INTEGER, false), p("w", TY_INTEGER, false)], None,
        Some("Write an INTEGER value to standard output, right-justified in a field of width `w`."));
    def_proc_doc(symtab, scope, "WriteCard", vec![p("n", TY_CARDINAL, false), p("w", TY_INTEGER, false)], None,
        Some("Write a CARDINAL value to standard output, right-justified in a field of width `w`."));
    def_proc_doc(symtab, scope, "WriteHex", vec![p("n", TY_CARDINAL, false), p("w", TY_INTEGER, false)], None,
        Some("Write a CARDINAL value in hexadecimal to standard output, right-justified in width `w`."));
    def_proc_doc(symtab, scope, "WriteOct", vec![p("n", TY_CARDINAL, false), p("w", TY_INTEGER, false)], None,
        Some("Write a CARDINAL value in octal to standard output, right-justified in width `w`."));
    def_proc_doc(symtab, scope, "WriteLn", vec![], None,
        Some("Write a newline to standard output."));
    def_proc_doc(symtab, scope, "ReadChar", vec![p("ch", TY_CHAR, true)], None,
        Some("Read a single character from standard input (alias for Read)."));
    def_proc_doc(symtab, scope, "WriteChar", vec![p("ch", TY_CHAR, false)], None,
        Some("Write a single character to standard output (alias for Write)."));
    def_var_doc(symtab, scope, "Done", TY_BOOLEAN,
        Some("TRUE if the last I/O operation succeeded."));
    def_proc_doc(symtab, scope, "OpenInput", vec![p("ext", TY_STRING, false)], None,
        Some("Prompt for an input filename and open it. Appends `ext` as extension if non-empty."));
    def_proc_doc(symtab, scope, "OpenOutput", vec![p("ext", TY_STRING, false)], None,
        Some("Prompt for an output filename and open it. Appends `ext` as extension if non-empty."));
    def_proc_doc(symtab, scope, "CloseInput", vec![], None,
        Some("Close the currently open input file."));
    def_proc_doc(symtab, scope, "CloseOutput", vec![], None,
        Some("Close the currently open output file."));
}

fn register_realinout(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "ReadReal", vec![p("r", TY_REAL, true)], None,
        Some("Read a REAL value from standard input. Sets `Done` to TRUE on success."));
    def_proc_doc(symtab, scope, "WriteReal", vec![p("r", TY_REAL, false), p("w", TY_INTEGER, false)], None,
        Some("Write a REAL value to standard output in general format, right-justified in width `w`."));
    def_proc_doc(symtab, scope, "WriteFixPt", vec![
        p("r", TY_REAL, false),
        p("w", TY_INTEGER, false),
        p("d", TY_INTEGER, false),
    ], None,
        Some("Write a REAL value in fixed-point notation with `d` decimal places, in a field of width `w`."));
    def_proc_doc(symtab, scope, "WriteRealOct", vec![p("r", TY_REAL, false)], None,
        Some("Write the internal (hexadecimal float) representation of a REAL value."));
    def_var_doc(symtab, scope, "Done", TY_BOOLEAN,
        Some("TRUE if the last I/O operation succeeded."));
}

fn register_mathlib(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "sqrt", vec![p("x", TY_REAL, false)], Some(TY_REAL),
        Some("Return the square root of `x`."));
    def_proc_doc(symtab, scope, "sin", vec![p("x", TY_REAL, false)], Some(TY_REAL),
        Some("Return the sine of `x` (radians)."));
    def_proc_doc(symtab, scope, "cos", vec![p("x", TY_REAL, false)], Some(TY_REAL),
        Some("Return the cosine of `x` (radians)."));
    def_proc_doc(symtab, scope, "arctan", vec![p("x", TY_REAL, false)], Some(TY_REAL),
        Some("Return the arctangent of `x` (result in radians)."));
    def_proc_doc(symtab, scope, "exp", vec![p("x", TY_REAL, false)], Some(TY_REAL),
        Some("Return e raised to the power `x`."));
    def_proc_doc(symtab, scope, "ln", vec![p("x", TY_REAL, false)], Some(TY_REAL),
        Some("Return the natural logarithm of `x`."));
    def_proc_doc(symtab, scope, "entier", vec![p("x", TY_REAL, false)], Some(TY_INTEGER),
        Some("Return the largest integer not greater than `x` (floor)."));
    def_proc_doc(symtab, scope, "real", vec![p("x", TY_INTEGER, false)], Some(TY_REAL),
        Some("Convert an INTEGER value to REAL."));
    def_proc_doc(symtab, scope, "Random", vec![], Some(TY_REAL),
        Some("Return a pseudo-random REAL in [0.0, 1.0)."));
    def_proc_doc(symtab, scope, "Randomize", vec![p("seed", TY_CARDINAL, false)], None,
        Some("Seed the pseudo-random number generator."));
}

fn register_strings(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "Assign", vec![p("src", TY_STRING, false), p("dst", TY_STRING, true)], None,
        Some("Copy string `src` into `dst`, truncating if `dst` is shorter."));
    def_proc_doc(symtab, scope, "Insert", vec![
        p("sub", TY_STRING, false),
        p("dst", TY_STRING, true),
        p("pos", TY_CARDINAL, false),
    ], None,
        Some("Insert string `sub` into `dst` at position `pos`. Truncates if result exceeds capacity."));
    def_proc_doc(symtab, scope, "Delete", vec![
        p("s", TY_STRING, true),
        p("pos", TY_CARDINAL, false),
        p("len", TY_CARDINAL, false),
    ], None,
        Some("Delete `len` characters from string `s` starting at position `pos`."));
    def_proc_doc(symtab, scope, "Pos", vec![
        p("sub", TY_STRING, false),
        p("s", TY_STRING, false),
    ], Some(TY_CARDINAL),
        Some("Return the position of the first occurrence of `sub` in `s`, or MAX(CARDINAL) if not found."));
    def_proc_doc(symtab, scope, "Length", vec![p("s", TY_STRING, false)], Some(TY_CARDINAL),
        Some("Return the length of string `s`."));
    def_proc_doc(symtab, scope, "Copy", vec![
        p("src", TY_STRING, false),
        p("pos", TY_CARDINAL, false),
        p("len", TY_CARDINAL, false),
        p("dst", TY_STRING, true),
    ], None,
        Some("Copy `len` characters from `src` starting at `pos` into `dst`."));
    def_proc_doc(symtab, scope, "Concat", vec![
        p("s1", TY_STRING, false),
        p("s2", TY_STRING, false),
        p("dst", TY_STRING, true),
    ], None,
        Some("Concatenate strings `s1` and `s2` into `dst`. Truncates if result exceeds capacity."));
    def_proc_doc(symtab, scope, "CompareStr", vec![
        p("s1", TY_STRING, false),
        p("s2", TY_STRING, false),
    ], Some(TY_INTEGER),
        Some("Compare strings `s1` and `s2`. Returns negative if s1 < s2, zero if equal, positive if s1 > s2."));
    def_proc_doc(symtab, scope, "CAPS", vec![p("s", TY_STRING, true)], None,
        Some("Convert all characters in `s` to upper case in place."));
}

fn register_storage(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "ALLOCATE", vec![
        p("p", TY_ADDRESS, true),
        p("size", TY_CARDINAL, false),
    ], None,
        Some("Allocate `size` bytes of memory and store the pointer in `p`. Called implicitly by NEW."));
    def_proc_doc(symtab, scope, "DEALLOCATE", vec![
        p("p", TY_ADDRESS, true),
        p("size", TY_CARDINAL, false),
    ], None,
        Some("Free the memory pointed to by `p` (must have been allocated with ALLOCATE). Called implicitly by DISPOSE."));
}

fn register_system(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // Types
    let _ = symtab.define(scope, Symbol {
        name: "WORD".to_string(),
        kind: SymbolKind::Type,
        typ: TY_WORD,
        exported: true,
        module: Some("SYSTEM".to_string()),
        loc: SourceLoc::default(),
        doc: Some("Machine word type. Compatible with all types of the same size.".to_string()),
    });
    let _ = symtab.define(scope, Symbol {
        name: "BYTE".to_string(),
        kind: SymbolKind::Type,
        typ: TY_BYTE,
        exported: true,
        module: Some("SYSTEM".to_string()),
        loc: SourceLoc::default(),
        doc: Some("Single byte type. Compatible with CHAR and small ordinal types.".to_string()),
    });
    let _ = symtab.define(scope, Symbol {
        name: "ADDRESS".to_string(),
        kind: SymbolKind::Type,
        typ: TY_ADDRESS,
        exported: true,
        module: Some("SYSTEM".to_string()),
        loc: SourceLoc::default(),
        doc: Some("Machine address type. Compatible with all pointer types.".to_string()),
    });

    // Procedures
    def_proc_doc(symtab, scope, "ADR", vec![p("x", TY_INTEGER, false)], Some(TY_ADDRESS),
        Some("Return the memory address of variable `x`."));
    def_proc_doc(symtab, scope, "TSIZE", vec![p("T", TY_INTEGER, false)], Some(TY_CARDINAL),
        Some("Return the size in bytes of type `T`."));
    def_proc_doc(symtab, scope, "NEWPROCESS", vec![
        p("p", TY_ADDRESS, false),
        p("a", TY_ADDRESS, false),
        p("n", TY_CARDINAL, false),
        p("new", TY_ADDRESS, true),
    ], None,
        Some("Create a new coroutine from procedure `p` with workspace `a` of `n` bytes."));
    def_proc_doc(symtab, scope, "TRANSFER", vec![
        p("from", TY_ADDRESS, true),
        p("to", TY_ADDRESS, true),
    ], None,
        Some("Transfer control from the current coroutine to coroutine `to`."));
    def_proc_doc(symtab, scope, "IOTRANSFER", vec![
        p("from", TY_ADDRESS, true),
        p("to", TY_ADDRESS, true),
        p("vec", TY_CARDINAL, false),
    ], None,
        Some("Transfer control to coroutine `to` and arrange for an interrupt on vector `vec` to transfer back."));
}

fn register_terminal(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "Read", vec![p("ch", TY_CHAR, true)], None,
        Some("Read a single character from the terminal."));
    def_proc_doc(symtab, scope, "Write", vec![p("ch", TY_CHAR, false)], None,
        Some("Write a single character to the terminal."));
    def_proc_doc(symtab, scope, "WriteString", vec![p("s", TY_STRING, false)], None,
        Some("Write a string to the terminal."));
    def_proc_doc(symtab, scope, "WriteLn", vec![], None,
        Some("Write a newline to the terminal."));
    def_var_doc(symtab, scope, "Done", TY_BOOLEAN,
        Some("TRUE if the last terminal I/O operation succeeded."));
}

fn register_filesystem(symtab: &mut SymbolTable, types: &mut TypeRegistry, scope: usize) {
    // File type (opaque)
    let file_type = types.register(crate::types::Type::Opaque {
        name: "File".to_string(),
        module: "FileSystem".to_string(),
    });
    let _ = symtab.define(scope, Symbol {
        name: "File".to_string(),
        kind: SymbolKind::Type,
        typ: file_type,
        exported: true,
        module: Some("FileSystem".to_string()),
        loc: SourceLoc::default(),
        doc: None,
    });

    def_proc_doc(symtab, scope, "Lookup", vec![
        p("f", file_type, true),
        p("name", TY_STRING, false),
        p("new", TY_BOOLEAN, false),
    ], None,
        Some("Open file `name`. If `new` is TRUE, create it if it doesn't exist. Sets `Done`."));
    def_proc_doc(symtab, scope, "Close", vec![p("f", file_type, true)], None,
        Some("Close an open file."));
    def_proc_doc(symtab, scope, "ReadChar", vec![
        p("f", file_type, true),
        p("ch", TY_CHAR, true),
    ], None,
        Some("Read a single character from file `f`. Sets `Done` to FALSE on EOF."));
    def_proc_doc(symtab, scope, "WriteChar", vec![
        p("f", file_type, true),
        p("ch", TY_CHAR, false),
    ], None,
        Some("Write a single character to file `f`."));
    def_var_doc(symtab, scope, "Done", TY_BOOLEAN,
        Some("TRUE if the last file operation succeeded."));
}

// ── ISO Standard Library Modules ──────────────────────────────────────

fn register_stextio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "WriteChar", vec![p("ch", TY_CHAR, false)], None,
        Some("Write a single character to the default output channel."));
    def_proc_doc(symtab, scope, "ReadChar", vec![p("ch", TY_CHAR, true)], None,
        Some("Read a single character from the default input channel."));
    def_proc_doc(symtab, scope, "WriteString", vec![p("s", TY_STRING, false)], None,
        Some("Write a string to the default output channel."));
    def_proc_doc(symtab, scope, "ReadString", vec![p("s", TY_STRING, true)], None,
        Some("Read a line of text from the default input channel."));
    def_proc_doc(symtab, scope, "WriteLn", vec![], None,
        Some("Write a newline to the default output channel."));
    def_proc_doc(symtab, scope, "SkipLine", vec![], None,
        Some("Skip to the end of the current input line."));
    def_proc_doc(symtab, scope, "ReadToken", vec![p("s", TY_STRING, true)], None,
        Some("Read a whitespace-delimited token from the default input channel."));
}

fn register_swholeio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "WriteInt", vec![p("n", TY_INTEGER, false), p("w", TY_CARDINAL, false)], None,
        Some("Write an INTEGER value right-justified in a field of width `w`."));
    def_proc_doc(symtab, scope, "ReadInt", vec![p("n", TY_INTEGER, true)], None,
        Some("Read an INTEGER value from the default input channel."));
    def_proc_doc(symtab, scope, "WriteCard", vec![p("n", TY_CARDINAL, false), p("w", TY_CARDINAL, false)], None,
        Some("Write a CARDINAL value right-justified in a field of width `w`."));
    def_proc_doc(symtab, scope, "ReadCard", vec![p("n", TY_CARDINAL, true)], None,
        Some("Read a CARDINAL value from the default input channel."));
}

fn register_srealio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "WriteFloat", vec![
        p("r", TY_REAL, false), p("sigFigs", TY_CARDINAL, false), p("w", TY_CARDINAL, false),
    ], None,
        Some("Write a REAL value in scientific notation with `sigFigs` significant digits, in width `w`."));
    def_proc_doc(symtab, scope, "WriteFixed", vec![
        p("r", TY_REAL, false), p("place", TY_INTEGER, false), p("w", TY_CARDINAL, false),
    ], None,
        Some("Write a REAL value in fixed-point notation with `place` decimal places, in width `w`."));
    def_proc_doc(symtab, scope, "WriteReal", vec![p("r", TY_REAL, false), p("w", TY_CARDINAL, false)], None,
        Some("Write a REAL value in general format, right-justified in width `w`."));
    def_proc_doc(symtab, scope, "ReadReal", vec![p("r", TY_REAL, true)], None,
        Some("Read a REAL value from the default input channel."));
}

fn register_slongio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "WriteFloat", vec![
        p("r", TY_LONGREAL, false), p("sigFigs", TY_CARDINAL, false), p("w", TY_CARDINAL, false),
    ], None,
        Some("Write a LONGREAL value in scientific notation with `sigFigs` significant digits, in width `w`."));
    def_proc_doc(symtab, scope, "WriteFixed", vec![
        p("r", TY_LONGREAL, false), p("place", TY_INTEGER, false), p("w", TY_CARDINAL, false),
    ], None,
        Some("Write a LONGREAL value in fixed-point notation with `place` decimal places, in width `w`."));
    def_proc_doc(symtab, scope, "WriteLongReal", vec![p("r", TY_LONGREAL, false), p("w", TY_CARDINAL, false)], None,
        Some("Write a LONGREAL value in general format, right-justified in width `w`."));
    def_proc_doc(symtab, scope, "ReadLongReal", vec![p("r", TY_LONGREAL, true)], None,
        Some("Read a LONGREAL value from the default input channel."));
}

fn register_sioresult(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // IOResult type and constants
    let _ = symtab.define(scope, Symbol {
        name: "ReadResults".to_string(),
        kind: SymbolKind::Type,
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
        doc: None,
    });
    let _ = symtab.define(scope, Symbol {
        name: "allRight".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(0)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
        doc: None,
    });
    let _ = symtab.define(scope, Symbol {
        name: "outOfRange".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(1)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
        doc: None,
    });
    let _ = symtab.define(scope, Symbol {
        name: "wrongFormat".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(2)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
        doc: None,
    });
    let _ = symtab.define(scope, Symbol {
        name: "endOfInput".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(3)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
        doc: None,
    });
}

// ── Args module ───────────────────────────────────────────────────────

fn register_args(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc_doc(symtab, scope, "ArgCount", vec![], Some(TY_CARDINAL),
        Some("Return the number of command-line arguments (including the program name)."));
    def_proc_doc(symtab, scope, "GetArg", vec![
        p("n", TY_CARDINAL, false),
        p("buf", TY_STRING, true),
    ], None,
        Some("Copy the `n`-th command-line argument into `buf` (0 = program name)."));
}

// ── BinaryIO module ──────────────────────────────────────────────────

fn register_binaryio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // FileHandle is represented as an opaque CARDINAL (actually a FILE* index)
    def_proc_doc(symtab, scope, "OpenRead", vec![
        p("name", TY_STRING, false),
        p("fh", TY_CARDINAL, true),
    ], None,
        Some("Open file `name` for binary reading. Returns a file handle in `fh`. Sets `Done`."));
    def_proc_doc(symtab, scope, "OpenWrite", vec![
        p("name", TY_STRING, false),
        p("fh", TY_CARDINAL, true),
    ], None,
        Some("Open (or create) file `name` for binary writing. Returns a file handle in `fh`. Sets `Done`."));
    def_proc_doc(symtab, scope, "Close", vec![p("fh", TY_CARDINAL, false)], None,
        Some("Close a binary file handle."));
    def_proc_doc(symtab, scope, "ReadByte", vec![
        p("fh", TY_CARDINAL, false),
        p("b", TY_CARDINAL, true),
    ], None,
        Some("Read a single byte from file `fh` into `b`. Sets `Done` to FALSE on EOF."));
    def_proc_doc(symtab, scope, "WriteByte", vec![
        p("fh", TY_CARDINAL, false),
        p("b", TY_CARDINAL, false),
    ], None,
        Some("Write a single byte `b` to file `fh`."));
    def_proc_doc(symtab, scope, "ReadBytes", vec![
        p("fh", TY_CARDINAL, false),
        p("buf", TY_STRING, true),
        p("n", TY_CARDINAL, false),
        p("actual", TY_CARDINAL, true),
    ], None,
        Some("Read up to `n` bytes from file `fh` into `buf`. `actual` receives the number of bytes read."));
    def_proc_doc(symtab, scope, "WriteBytes", vec![
        p("fh", TY_CARDINAL, false),
        p("buf", TY_STRING, false),
        p("n", TY_CARDINAL, false),
    ], None,
        Some("Write `n` bytes from `buf` to file `fh`."));
    def_proc_doc(symtab, scope, "FileSize", vec![
        p("fh", TY_CARDINAL, false),
        p("size", TY_CARDINAL, true),
    ], None,
        Some("Get the size of file `fh` in bytes."));
    def_proc_doc(symtab, scope, "Seek", vec![
        p("fh", TY_CARDINAL, false),
        p("pos", TY_CARDINAL, false),
    ], None,
        Some("Set the file position of `fh` to byte offset `pos`."));
    def_proc_doc(symtab, scope, "Tell", vec![
        p("fh", TY_CARDINAL, false),
        p("pos", TY_CARDINAL, true),
    ], None,
        Some("Get the current file position of `fh`."));
    def_proc_doc(symtab, scope, "IsEOF", vec![
        p("fh", TY_CARDINAL, false),
    ], Some(TY_BOOLEAN),
        Some("Return TRUE if file `fh` is at end-of-file."));
    def_var_doc(symtab, scope, "Done", TY_BOOLEAN,
        Some("TRUE if the last BinaryIO operation succeeded."));
}

// ── Modula-2+ Concurrency Modules ────────────────────────────────────

fn register_thread(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // T is an opaque type (pointer to m2_Thread struct) — use ADDRESS
    let _ = symtab.define(scope, Symbol {
        name: "T".to_string(),
        kind: SymbolKind::Type,
        typ: TY_ADDRESS,
        exported: true,
        module: None,
        loc: SourceLoc::default(),
        doc: Some("Opaque thread handle type.".to_string()),
    });
    def_proc_doc(symtab, scope, "Fork", vec![p("p", TY_ADDRESS, false)], Some(TY_ADDRESS),
        Some("Create a new thread that executes parameterless procedure `p`. Returns a thread handle."));
    def_proc_doc(symtab, scope, "Join", vec![p("t", TY_ADDRESS, false)], None,
        Some("Wait for thread `t` to finish execution."));
    def_proc_doc(symtab, scope, "Self", vec![], Some(TY_ADDRESS),
        Some("Return the handle of the currently executing thread."));
    def_proc_doc(symtab, scope, "Alert", vec![p("t", TY_ADDRESS, false)], None,
        Some("Set the alert flag on thread `t`. The thread can check this via TestAlert."));
    def_proc_doc(symtab, scope, "TestAlert", vec![], Some(TY_BOOLEAN),
        Some("Check and clear the current thread's alert flag. Returns TRUE if it was set."));
}

fn register_mutex(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // T is opaque (pointer to pthread_mutex_t) — use ADDRESS
    let _ = symtab.define(scope, Symbol {
        name: "T".to_string(),
        kind: SymbolKind::Type,
        typ: TY_ADDRESS,
        exported: true,
        module: None,
        loc: SourceLoc::default(),
        doc: Some("Opaque mutex handle type.".to_string()),
    });
    def_proc_doc(symtab, scope, "New", vec![], Some(TY_ADDRESS),
        Some("Create and return a new mutex."));
    def_proc_doc(symtab, scope, "Lock", vec![p("m", TY_ADDRESS, false)], None,
        Some("Acquire the mutex `m`. Blocks if already held by another thread."));
    def_proc_doc(symtab, scope, "Unlock", vec![p("m", TY_ADDRESS, false)], None,
        Some("Release the mutex `m`."));
    def_proc_doc(symtab, scope, "Free", vec![p("m", TY_ADDRESS, false)], None,
        Some("Destroy and free the mutex `m`."));
}

fn register_condition(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // T is opaque (pointer to pthread_cond_t) — use ADDRESS
    let _ = symtab.define(scope, Symbol {
        name: "T".to_string(),
        kind: SymbolKind::Type,
        typ: TY_ADDRESS,
        exported: true,
        module: None,
        loc: SourceLoc::default(),
        doc: Some("Opaque condition variable handle type.".to_string()),
    });
    def_proc_doc(symtab, scope, "New", vec![], Some(TY_ADDRESS),
        Some("Create and return a new condition variable."));
    def_proc_doc(symtab, scope, "Wait", vec![p("c", TY_ADDRESS, false), p("m", TY_ADDRESS, false)], None,
        Some("Atomically release mutex `m` and wait on condition `c`. Re-acquires `m` on wakeup."));
    def_proc_doc(symtab, scope, "Signal", vec![p("c", TY_ADDRESS, false)], None,
        Some("Wake one thread waiting on condition `c`."));
    def_proc_doc(symtab, scope, "Broadcast", vec![p("c", TY_ADDRESS, false)], None,
        Some("Wake all threads waiting on condition `c`."));
    def_proc_doc(symtab, scope, "Free", vec![p("c", TY_ADDRESS, false)], None,
        Some("Destroy and free the condition variable `c`."));
}

/// Generate C runtime support code for stdlib modules
pub fn generate_runtime_header() -> String {
    r#"/* Modula-2 Runtime Support */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include <ctype.h>
#include <limits.h>
#include <float.h>
#include <setjmp.h>

/* Command-line argument storage */
static int m2_argc = 0;
static char **m2_argv = NULL;

/* ISO Modula-2 exception handling support */
static jmp_buf m2_exception_buf;
static int m2_exception_code = 0;
static int m2_exception_active = 0;

/* Modula-2+ enhanced exception handling (setjmp/longjmp frame stack) */
typedef struct m2_ExcFrame {
    jmp_buf buf;
    struct m2_ExcFrame *prev;
    int exception_id;
    const char *exception_name;
    void *exception_arg;
} m2_ExcFrame;

static __thread m2_ExcFrame *m2_exc_stack = NULL;

/* Stack-based exception frame macros — no heap allocation.
   Usage:  m2_ExcFrame _ef;
           M2_TRY(_ef) { body; M2_ENDTRY(_ef); }
           M2_CATCH { M2_ENDTRY(_ef); handlers; }           */
#define M2_TRY(frame) \
    (frame).prev = m2_exc_stack; \
    (frame).exception_id = 0; \
    (frame).exception_name = NULL; \
    (frame).exception_arg = NULL; \
    m2_exc_stack = &(frame); \
    if (setjmp((frame).buf) == 0)

#define M2_CATCH else

#define M2_ENDTRY(frame) \
    m2_exc_stack = (frame).prev

static inline void m2_raise(int id, const char *name, void *arg) {
    if (m2_exc_stack) {
        m2_exc_stack->exception_id = id;
        m2_exc_stack->exception_name = name;
        m2_exc_stack->exception_arg = arg;
        longjmp(m2_exc_stack->buf, id ? id : 1);
    }
    /* Fallback to ISO exception mechanism */
    if (m2_exception_active) {
        m2_exception_code = id ? id : 1;
        longjmp(m2_exception_buf, m2_exception_code);
    }
    /* No handler — terminate */
    fprintf(stderr, "Unhandled exception: %s (id=%d)\n", name ? name : "unknown", id);
    exit(1);
}

/* Runtime type information (for TYPECASE / OBJECT) */
typedef struct M2_TypeDesc {
    uint32_t   type_id;
    const char *type_name;
    struct M2_TypeDesc *parent;
    uint32_t   depth;
} M2_TypeDesc;

/* Allocation header prepended before payload for typed REF/OBJECT allocations */
typedef struct M2_RefHeader {
#ifdef M2_RTTI_DEBUG
    uint32_t magic;   /* 0x4D325246 ("M2RF") */
    uint32_t flags;   /* 0 = live, 0xDEADDEAD = freed */
#endif
    M2_TypeDesc *td;
} M2_RefHeader;

#define M2_REFHEADER_MAGIC 0x4D325246u

/* Modula-2+ Thread support (pthreads) */
#ifdef M2_USE_THREADS
#include <pthread.h>
typedef struct m2_Thread {
    pthread_t handle;
    int alerted;
    pthread_mutex_t alert_mu;
} m2_Thread;

static __thread m2_Thread *m2_current_thread = NULL;

/* Thread.Fork — create a new thread from a parameterless procedure */
typedef void (*m2_ThreadProc)(void);
struct m2_thread_start_arg { m2_ThreadProc proc; m2_Thread *self; };

static void *m2_thread_start(void *arg) {
    struct m2_thread_start_arg *a = (struct m2_thread_start_arg *)arg;
    m2_current_thread = a->self;
    a->proc();
    free(a);
    return NULL;
}

static m2_Thread *m2_Thread_Fork(m2_ThreadProc proc) {
    m2_Thread *t = (m2_Thread *)malloc(sizeof(m2_Thread));
    t->alerted = 0;
    pthread_mutex_init(&t->alert_mu, NULL);
    struct m2_thread_start_arg *arg = (struct m2_thread_start_arg *)malloc(sizeof(struct m2_thread_start_arg));
    arg->proc = proc;
    arg->self = t;
    pthread_create(&t->handle, NULL, m2_thread_start, arg);
    return t;
}

static void m2_Thread_Join(m2_Thread *t) {
    pthread_join(t->handle, NULL);
}

static m2_Thread *m2_Thread_Self(void) {
    return m2_current_thread;
}

static void m2_Thread_Alert(m2_Thread *t) {
    pthread_mutex_lock(&t->alert_mu);
    t->alerted = 1;
    pthread_mutex_unlock(&t->alert_mu);
}

static int m2_Thread_TestAlert(void) {
    if (!m2_current_thread) return 0;
    pthread_mutex_lock(&m2_current_thread->alert_mu);
    int a = m2_current_thread->alerted;
    m2_current_thread->alerted = 0;
    pthread_mutex_unlock(&m2_current_thread->alert_mu);
    return a;
}

/* Mutex module */
typedef pthread_mutex_t *m2_Mutex_T;

static m2_Mutex_T m2_Mutex_New(void) {
    pthread_mutex_t *m = (pthread_mutex_t *)malloc(sizeof(pthread_mutex_t));
    pthread_mutex_init(m, NULL);
    return m;
}

static void m2_Mutex_Lock(m2_Mutex_T m) { pthread_mutex_lock(m); }
static void m2_Mutex_Unlock(m2_Mutex_T m) { pthread_mutex_unlock(m); }
static void m2_Mutex_Free(m2_Mutex_T m) { pthread_mutex_destroy(m); free(m); }

/* Condition module */
typedef pthread_cond_t *m2_Condition_T;

static m2_Condition_T m2_Condition_New(void) {
    pthread_cond_t *c = (pthread_cond_t *)malloc(sizeof(pthread_cond_t));
    pthread_cond_init(c, NULL);
    return c;
}

static void m2_Condition_Wait(m2_Condition_T c, m2_Mutex_T m) { pthread_cond_wait(c, m); }
static void m2_Condition_Signal(m2_Condition_T c) { pthread_cond_signal(c); }
static void m2_Condition_Broadcast(m2_Condition_T c) { pthread_cond_broadcast(c); }
static void m2_Condition_Free(m2_Condition_T c) { pthread_cond_destroy(c); free(c); }
#endif /* M2_USE_THREADS */

/* Modula-2+ Garbage Collection support (Boehm GC) */
#if defined(M2_USE_GC) && __has_include(<gc/gc.h>)
#include <gc/gc.h>
#else
/* Fallback: use malloc when GC is not available */
#ifdef M2_USE_GC
#undef M2_USE_GC
#endif
#define GC_MALLOC(sz) malloc(sz)
#define GC_REALLOC(p, sz) realloc(p, sz)
#define GC_FREE(p) free(p)
static inline void GC_INIT(void) {}
#endif

/* Allocate a GC-traced REF/OBJECT with M2_RefHeader prepended before payload */
static inline void *M2_ref_alloc(size_t payload_size, M2_TypeDesc *td) {
    M2_RefHeader *hdr = (M2_RefHeader *)GC_MALLOC(sizeof(M2_RefHeader) + payload_size);
    if (!hdr) { fprintf(stderr, "M2_ref_alloc: out of memory\n"); exit(1); }
#ifdef M2_RTTI_DEBUG
    hdr->magic = M2_REFHEADER_MAGIC;
    hdr->flags = 0;
#endif
    hdr->td = td;
    return (void *)(hdr + 1); /* return pointer to payload (past header) */
}

/* Recover the type descriptor from a typed REF/REFANY payload pointer.
   Returns NULL if ref is NULL or (in debug mode) if the header is invalid. */
static inline M2_TypeDesc *M2_TYPEOF(void *ref) {
    if (!ref) return NULL;
    M2_RefHeader *hdr = ((M2_RefHeader *)ref) - 1;
#ifdef M2_RTTI_DEBUG
    if (hdr->magic != M2_REFHEADER_MAGIC) return NULL;
    if (hdr->flags == 0xDEADDEADu) {
        fprintf(stderr, "M2_TYPEOF: use-after-free detected\n");
        return NULL;
    }
#endif
    return hdr->td;
}

/* Check if a payload's type is (or inherits from) a target type descriptor.
   Returns 1 if match, 0 otherwise. Safe with NULL payloads. */
static inline int M2_ISA(void *payload, M2_TypeDesc *target) {
    M2_TypeDesc *td = M2_TYPEOF(payload);
    if (!td || !target) return 0;
    if (td->depth < target->depth) return 0; /* early-out: can't be a subtype */
    while (td) {
        if (td == target) return 1;
        td = td->parent;
    }
    return 0;
}

/* Narrow: returns payload if it matches target type, otherwise raises an exception */
static inline void *M2_NARROW(void *payload, M2_TypeDesc *target) {
    if (M2_ISA(payload, target)) return payload;
    m2_raise(99, "NarrowFault", NULL);
    return NULL; /* unreachable */
}

/* Free a typed REF object — poisons header in debug mode */
static inline void M2_ref_free(void *payload) {
    if (!payload) return;
    M2_RefHeader *hdr = ((M2_RefHeader *)payload) - 1;
#ifdef M2_RTTI_DEBUG
    hdr->flags = 0xDEADDEADu;
#endif
    GC_FREE(hdr);
}

/* PIM4 DIV: floored division (truncates toward negative infinity) */
static inline int32_t m2_div(int32_t a, int32_t b) {
    int32_t q = a / b;
    int32_t r = a % b;
    if ((r != 0) && ((r ^ b) < 0)) q--;
    return q;
}

/* PIM4 MOD: result is always non-negative (when b > 0) */
static inline int32_t m2_mod(int32_t a, int32_t b) {
    int32_t r = a % b;
    if (r < 0) r += (b > 0 ? b : -b);
    return r;
}

/* PIM4 DIV64: 64-bit floored division for LONGINT */
static inline int64_t m2_div64(int64_t a, int64_t b) {
    int64_t q = a / b;
    int64_t r = a % b;
    if ((r != 0) && ((r ^ b) < 0)) q--;
    return q;
}

/* PIM4 MOD64: 64-bit modulo for LONGINT */
static inline int64_t m2_mod64(int64_t a, int64_t b) {
    int64_t r = a % b;
    if (r < 0) r += (b > 0 ? b : -b);
    return r;
}

/* ISO Modula-2 COMPLEX types */
typedef struct { float re, im; } m2_COMPLEX;
typedef struct { double re, im; } m2_LONGCOMPLEX;

static inline m2_COMPLEX m2_complex_add(m2_COMPLEX a, m2_COMPLEX b) {
    return (m2_COMPLEX){ a.re + b.re, a.im + b.im };
}
static inline m2_COMPLEX m2_complex_sub(m2_COMPLEX a, m2_COMPLEX b) {
    return (m2_COMPLEX){ a.re - b.re, a.im - b.im };
}
static inline m2_COMPLEX m2_complex_mul(m2_COMPLEX a, m2_COMPLEX b) {
    return (m2_COMPLEX){ a.re*b.re - a.im*b.im, a.re*b.im + a.im*b.re };
}
static inline m2_COMPLEX m2_complex_div(m2_COMPLEX a, m2_COMPLEX b) {
    float d = b.re*b.re + b.im*b.im;
    return (m2_COMPLEX){ (a.re*b.re + a.im*b.im)/d, (a.im*b.re - a.re*b.im)/d };
}
static inline int m2_complex_eq(m2_COMPLEX a, m2_COMPLEX b) {
    return a.re == b.re && a.im == b.im;
}
static inline m2_COMPLEX m2_complex_neg(m2_COMPLEX a) {
    return (m2_COMPLEX){ -a.re, -a.im };
}
static inline float m2_complex_abs(m2_COMPLEX a) {
    return sqrtf(a.re*a.re + a.im*a.im);
}
static inline m2_LONGCOMPLEX m2_lcomplex_add(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return (m2_LONGCOMPLEX){ a.re + b.re, a.im + b.im };
}
static inline m2_LONGCOMPLEX m2_lcomplex_sub(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return (m2_LONGCOMPLEX){ a.re - b.re, a.im - b.im };
}
static inline m2_LONGCOMPLEX m2_lcomplex_mul(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return (m2_LONGCOMPLEX){ a.re*b.re - a.im*b.im, a.re*b.im + a.im*b.re };
}
static inline m2_LONGCOMPLEX m2_lcomplex_div(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    double d = b.re*b.re + b.im*b.im;
    return (m2_LONGCOMPLEX){ (a.re*b.re + a.im*b.im)/d, (a.im*b.re - a.re*b.im)/d };
}
static inline int m2_lcomplex_eq(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return a.re == b.re && a.im == b.im;
}
static inline m2_LONGCOMPLEX m2_lcomplex_neg(m2_LONGCOMPLEX a) {
    return (m2_LONGCOMPLEX){ -a.re, -a.im };
}
static inline double m2_lcomplex_abs(m2_LONGCOMPLEX a) {
    return sqrt(a.re*a.re + a.im*a.im);
}

/* Built-in MAX/MIN - type-generic via macros */
#define m2_max_INTEGER INT32_MAX
#define m2_max_CARDINAL UINT32_MAX
#define m2_max_CHAR 255
#define m2_max_BOOLEAN 1
#define m2_max_REAL FLT_MAX
#define m2_max_LONGREAL DBL_MAX
#define m2_max_BITSET 31
#define m2_max_LONGINT INT64_MAX
#define m2_max_LONGCARD UINT64_MAX
#define m2_min_INTEGER INT32_MIN
#define m2_min_CARDINAL 0
#define m2_min_CHAR 0
#define m2_min_BOOLEAN 0
#define m2_min_REAL FLT_MIN
#define m2_min_LONGREAL DBL_MIN
#define m2_min_BITSET 0
#define m2_min_LONGINT INT64_MIN
#define m2_min_LONGCARD 0
#define m2_max(T) m2_max_##T
#define m2_min(T) m2_min_##T

/* ISO SYSTEM.SHIFT — positive n shifts left, negative shifts right, vacated bits = 0 */
static inline uint32_t m2_shift(uint32_t val, int32_t n) {
    if (n == 0) return val;
    if (n > 0) return (n >= 32) ? 0u : (val << n);
    n = -n;
    return (n >= 32) ? 0u : (val >> n);
}
/* ISO SYSTEM.ROTATE — positive n rotates left, negative rotates right */
static inline uint32_t m2_rotate(uint32_t val, int32_t n) {
    n = n % 32;
    if (n < 0) n += 32;
    if (n == 0) return val;
    return (val << n) | (val >> (32 - n));
}

/* InOut module */
static int m2_InOut_Done = 1;
static void m2_WriteString(const char *s) { printf("%s", s); }
static void m2_WriteLn(void) { printf("\n"); }
static void m2_WriteInt(int32_t n, int32_t w) { printf("%*d", (int)w, (int)n); }
static void m2_WriteCard(uint32_t n, int32_t w) { printf("%*u", (int)w, (unsigned)n); }
static void m2_WriteHex(uint32_t n, int32_t w) { printf("%*X", (int)w, (unsigned)n); }
static void m2_WriteOct(uint32_t n, int32_t w) { printf("%*o", (int)w, (unsigned)n); }
static void m2_Write(char ch) { putchar(ch); }
static void m2_Read(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; m2_InOut_Done = (c != EOF); }
static void m2_ReadString(char *s) { m2_InOut_Done = (scanf("%s", s) == 1); }
static void m2_ReadInt(int32_t *n) { m2_InOut_Done = (scanf("%d", n) == 1); }
static void m2_ReadCard(uint32_t *n) { m2_InOut_Done = (scanf("%u", n) == 1); }

static FILE *m2_InFile = NULL;
static FILE *m2_OutFile = NULL;
static void m2_OpenInput(const char *ext) {
    char name[256];
    printf("Input file: "); scanf("%255s", name);
    if (ext && ext[0]) { strcat(name, "."); strcat(name, ext); }
    m2_InFile = fopen(name, "r");
    m2_InOut_Done = (m2_InFile != NULL);
}
static void m2_OpenOutput(const char *ext) {
    char name[256];
    printf("Output file: "); scanf("%255s", name);
    if (ext && ext[0]) { strcat(name, "."); strcat(name, ext); }
    m2_OutFile = fopen(name, "w");
    m2_InOut_Done = (m2_OutFile != NULL);
}
static void m2_CloseInput(void) { if (m2_InFile) { fclose(m2_InFile); m2_InFile = NULL; } }
static void m2_CloseOutput(void) { if (m2_OutFile) { fclose(m2_OutFile); m2_OutFile = NULL; } }

/* RealInOut module */
static int m2_RealInOut_Done = 1;
static void m2_ReadReal(float *r) { m2_RealInOut_Done = (scanf("%f", r) == 1); }
static void m2_WriteReal(float r, int32_t w) { printf("%*g", (int)w, (double)r); }
static void m2_WriteFixPt(float r, int32_t w, int32_t d) { printf("%*.*f", (int)w, (int)d, (double)r); }
static void m2_WriteRealOct(float r) { printf("%.8A", (double)r); }

/* MathLib — Random/Randomize */
static float m2_Random(void) { return (float)rand() / ((float)RAND_MAX + 1.0f); }
static void m2_Randomize(uint32_t seed) { srand(seed); }

/* Storage module */
static void m2_ALLOCATE(void **p, uint32_t size) { *p = malloc(size); }
static void m2_DEALLOCATE(void **p, uint32_t size) { free(*p); *p = NULL; (void)size; }

/* Strings module — bounded, always NUL-terminates, truncates on overflow.
   When both source length and capacity are compile-time constants (e.g. a
   string literal assigned to a fixed-size array), the branch resolves at
   compile time and the copy becomes a single memcpy/strcpy intrinsic that
   downstream optimisations (constant-folding of strcmp, etc.) can see through. */
static inline __attribute__((always_inline)) void m2_Strings_Assign(const char *src, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = __builtin_strlen(src);
    if (__builtin_constant_p(slen) && __builtin_constant_p(cap) && slen < cap) {
        __builtin_memcpy(dst, src, slen + 1);
    } else {
        if (slen >= cap) slen = cap - 1;
        __builtin_memcpy(dst, src, slen);
        dst[slen] = '\0';
    }
}
static void m2_Strings_Insert(const char *sub, char *dst, uint32_t dst_high, uint32_t pos) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(sub), dlen = strlen(dst);
    if (pos > dlen) pos = (uint32_t)dlen;
    size_t new_len = dlen + slen;
    if (new_len >= cap) new_len = cap - 1;
    /* how much of the tail after pos can we keep? */
    size_t tail_dst = pos + slen;
    size_t tail_keep = (tail_dst < new_len) ? new_len - tail_dst : 0;
    if (tail_keep > 0)
        memmove(dst + tail_dst, dst + pos, tail_keep);
    /* how much of sub fits? */
    size_t sub_copy = slen;
    if (pos + sub_copy > new_len) sub_copy = new_len - pos;
    if (sub_copy > 0)
        memcpy(dst + pos, sub, sub_copy);
    dst[new_len] = '\0';
}
static void m2_Strings_Delete(char *s, uint32_t s_high, uint32_t pos, uint32_t len) {
    size_t slen = strlen(s);
    (void)s_high; /* delete only shrinks — can never overflow */
    if (pos >= slen) return;
    if (pos + len > slen) len = (uint32_t)(slen - pos);
    memmove(s + pos, s + pos + len, slen - pos - len + 1);
}
static uint32_t m2_Strings_Pos(const char *sub, const char *s) {
    const char *p = strstr(s, sub);
    return p ? (uint32_t)(p - s) : UINT32_MAX;
}
static uint32_t m2_Strings_Length(const char *s) { return (uint32_t)strlen(s); }
static void m2_Strings_Copy(const char *src, uint32_t pos, uint32_t len, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(src);
    if (pos >= slen) { dst[0] = '\0'; return; }
    if (pos + len > slen) len = (uint32_t)(slen - pos);
    if (len >= cap) len = (uint32_t)(cap - 1);
    memcpy(dst, src + pos, len);
    dst[len] = '\0';
}
static void m2_Strings_Concat(const char *s1, const char *s2, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t len1 = strlen(s1), len2 = strlen(s2);
    if (len1 >= cap) len1 = cap - 1;
    memcpy(dst, s1, len1);
    size_t rem = cap - 1 - len1;
    if (len2 > rem) len2 = rem;
    memcpy(dst + len1, s2, len2);
    dst[len1 + len2] = '\0';
}
static int32_t m2_Strings_CompareStr(const char *s1, const char *s2) { return (int32_t)strcmp(s1, s2); }
static void m2_Strings_CAPS(char *s, uint32_t s_high) { for (uint32_t i = 0; i <= s_high && s[i]; i++) s[i] = (char)toupper((unsigned char)s[i]); }

/* Terminal module */
static int m2_Terminal_Done = 1;
static void m2_Terminal_Read(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; m2_Terminal_Done = (c != EOF); }
static void m2_Terminal_Write(char ch) { putchar(ch); }
static void m2_Terminal_WriteString(const char *s) { printf("%s", s); }
static void m2_Terminal_WriteLn(void) { printf("\n"); }

/* FileSystem module */
typedef FILE *m2_File;
static int m2_FileSystem_Done = 1;
static void m2_Lookup(m2_File *f, const char *name, int newFile) {
    *f = fopen(name, newFile ? "w+" : "r+");
    if (!*f && !newFile) *f = fopen(name, "r");
    m2_FileSystem_Done = (*f != NULL);
}
static void m2_Close(m2_File *f) { if (*f) { fclose(*f); *f = NULL; } }
static void m2_ReadChar(m2_File *f, char *ch) {
    int c = fgetc(*f);
    *ch = (c == EOF) ? '\0' : (char)c;
    m2_FileSystem_Done = (c != EOF);
}
static void m2_WriteChar(m2_File *f, char ch) {
    fputc(ch, *f);
}

/* SYSTEM module */
#define m2_ADR(x) ((void *)&(x))
#define m2_TSIZE(T) ((uint32_t)sizeof(T))

/* ISO STextIO module */
static void m2_STextIO_WriteChar(char ch) { putchar(ch); }
static void m2_STextIO_ReadChar(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; }
static void m2_STextIO_WriteString(const char *s) { printf("%s", s); }
static void m2_STextIO_ReadString(char *s, uint32_t s_high) {
    if (fgets(s, (int)(s_high + 1), stdin) == NULL) s[0] = '\0';
    /* strip trailing newline */
    size_t len = strlen(s);
    if (len > 0 && s[len-1] == '\n') s[len-1] = '\0';
}
static void m2_STextIO_WriteLn(void) { putchar('\n'); }
static void m2_STextIO_SkipLine(void) { int c; while ((c = getchar()) != '\n' && c != EOF); }
static void m2_STextIO_ReadToken(char *s, uint32_t s_high) { m2_STextIO_ReadString(s, s_high); }

/* ISO SWholeIO module */
static void m2_SWholeIO_WriteInt(int32_t n, uint32_t w) { printf("%*d", (int)w, (int)n); }
static void m2_SWholeIO_ReadInt(int32_t *n) { scanf("%d", (int *)n); }
static void m2_SWholeIO_WriteCard(uint32_t n, uint32_t w) { printf("%*u", (int)w, (unsigned)n); }
static void m2_SWholeIO_ReadCard(uint32_t *n) { scanf("%u", (unsigned *)n); }

/* ISO SRealIO module */
static void m2_SRealIO_WriteFloat(float r, uint32_t sigFigs, uint32_t w) {
    printf("%*.*e", (int)w, (int)sigFigs, (double)r);
}
static void m2_SRealIO_WriteFixed(float r, int32_t place, uint32_t w) {
    printf("%*.*f", (int)w, (int)place, (double)r);
}
static void m2_SRealIO_WriteReal(float r, uint32_t w) { printf("%*g", (int)w, (double)r); }
static void m2_SRealIO_ReadReal(float *r) { double d; scanf("%lf", &d); *r = (float)d; }

/* ISO SLongIO module */
static void m2_SLongIO_WriteFloat(double r, uint32_t sigFigs, uint32_t w) {
    printf("%*.*e", (int)w, (int)sigFigs, r);
}
static void m2_SLongIO_WriteFixed(double r, int32_t place, uint32_t w) {
    printf("%*.*f", (int)w, (int)place, r);
}
static void m2_SLongIO_WriteLongReal(double r, uint32_t w) { printf("%*g", (int)w, r); }
static void m2_SLongIO_ReadLongReal(double *r) { scanf("%lf", r); }

/* Args module */
static uint32_t m2_Args_ArgCount(void) { return (uint32_t)m2_argc; }
static void m2_Args_GetArg(uint32_t n, char *buf, uint32_t buf_high) {
    (void)buf_high;
    if ((int)n < m2_argc) {
        strncpy(buf, m2_argv[n], buf_high + 1);
        buf[buf_high] = '\0';
    } else {
        buf[0] = '\0';
    }
}

/* BinaryIO module - file handle table using FILE* pointers */
#define M2_MAX_FILES 32
static FILE *m2_bio_files[M2_MAX_FILES];
static int m2_bio_init = 0;
static int m2_BinaryIO_Done = 1;

static void m2_bio_ensure_init(void) {
    if (!m2_bio_init) {
        for (int i = 0; i < M2_MAX_FILES; i++) m2_bio_files[i] = NULL;
        m2_bio_init = 1;
    }
}

static int m2_bio_alloc(void) {
    m2_bio_ensure_init();
    for (int i = 0; i < M2_MAX_FILES; i++) {
        if (m2_bio_files[i] == NULL) return i;
    }
    return -1;
}

static void m2_BinaryIO_OpenRead(const char *name, uint32_t *fh) {
    int slot = m2_bio_alloc();
    if (slot < 0) { m2_BinaryIO_Done = 0; *fh = 0; return; }
    m2_bio_files[slot] = fopen(name, "rb");
    if (m2_bio_files[slot]) { *fh = (uint32_t)(slot + 1); m2_BinaryIO_Done = 1; }
    else { *fh = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_OpenWrite(const char *name, uint32_t *fh) {
    int slot = m2_bio_alloc();
    if (slot < 0) { m2_BinaryIO_Done = 0; *fh = 0; return; }
    m2_bio_files[slot] = fopen(name, "wb");
    if (m2_bio_files[slot]) { *fh = (uint32_t)(slot + 1); m2_BinaryIO_Done = 1; }
    else { *fh = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_Close(uint32_t fh) {
    m2_bio_ensure_init();
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fclose(m2_bio_files[fh-1]);
        m2_bio_files[fh-1] = NULL;
    }
}

static void m2_BinaryIO_ReadByte(uint32_t fh, uint32_t *b) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        int c = fgetc(m2_bio_files[fh-1]);
        if (c == EOF) { *b = 0; m2_BinaryIO_Done = 0; }
        else { *b = (uint32_t)(unsigned char)c; m2_BinaryIO_Done = 1; }
    } else { *b = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_WriteByte(uint32_t fh, uint32_t b) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fputc((unsigned char)(b & 0xFF), m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_ReadBytes(uint32_t fh, char *buf, uint32_t n, uint32_t *actual) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        *actual = (uint32_t)fread(buf, 1, n, m2_bio_files[fh-1]);
        m2_BinaryIO_Done = (*actual > 0) ? 1 : 0;
    } else { *actual = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_WriteBytes(uint32_t fh, const char *buf, uint32_t n) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fwrite(buf, 1, n, m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_FileSize(uint32_t fh, uint32_t *size) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        long cur = ftell(m2_bio_files[fh-1]);
        fseek(m2_bio_files[fh-1], 0, SEEK_END);
        *size = (uint32_t)ftell(m2_bio_files[fh-1]);
        fseek(m2_bio_files[fh-1], cur, SEEK_SET);
        m2_BinaryIO_Done = 1;
    } else { *size = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_Seek(uint32_t fh, uint32_t pos) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fseek(m2_bio_files[fh-1], (long)pos, SEEK_SET);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_Tell(uint32_t fh, uint32_t *pos) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        *pos = (uint32_t)ftell(m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { *pos = 0; m2_BinaryIO_Done = 0; }
}

static int m2_BinaryIO_IsEOF(uint32_t fh) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        return feof(m2_bio_files[fh-1]) ? 1 : 0;
    }
    return 1;
}

"#
    .to_string()
}

/// Generate a standalone C runtime file for linking with LLVM IR output.
/// All functions have external linkage (no `static`) so they can be called
/// from LLVM-generated code.
pub fn generate_llvm_runtime_c() -> String {
    r#"/* Modula-2 LLVM Runtime Support — standalone linkable version */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include <ctype.h>
#include <limits.h>
#include <float.h>

/* Command-line argument storage */
int m2_argc = 0;
char **m2_argv = NULL;

/* PIM4 floored DIV and MOD */
int32_t m2_div(int32_t a, int32_t b) {
    int32_t q = a / b;
    if ((a % b != 0) && ((a ^ b) < 0)) q--;
    return q;
}
int32_t m2_mod(int32_t a, int32_t b) {
    int32_t r = a % b;
    if (r < 0) r += (b > 0 ? b : -b);
    return r;
}
int64_t m2_div64(int64_t a, int64_t b) {
    int64_t q = a / b;
    if ((a % b != 0) && ((a ^ b) < 0)) q--;
    return q;
}
int64_t m2_mod64(int64_t a, int64_t b) {
    int64_t r = a % b;
    if (r < 0) r += (b > 0 ? b : -b);
    return r;
}

/* SHIFT / ROTATE */
uint32_t m2_shift(uint32_t val, int32_t n) {
    if (n == 0) return val;
    if (n > 0) return (n >= 32) ? 0u : (val << n);
    n = -n;
    return (n >= 32) ? 0u : (val >> n);
}
uint32_t m2_rotate(uint32_t val, int32_t n) {
    n = n % 32;
    if (n < 0) n += 32;
    if (n == 0) return val;
    return (val << n) | (val >> (32 - n));
}

/* InOut module */
int m2_InOut_Done = 1;
void m2_WriteString(const char *s) { printf("%s", s); }
void m2_WriteLn(void) { printf("\n"); }
void m2_WriteInt(int32_t n, int32_t w) { printf("%*d", (int)w, (int)n); }
void m2_WriteCard(uint32_t n, int32_t w) { printf("%*u", (int)w, (unsigned)n); }
void m2_WriteHex(uint32_t n, int32_t w) { printf("%*X", (int)w, (unsigned)n); }
void m2_WriteOct(uint32_t n, int32_t w) { printf("%*o", (int)w, (unsigned)n); }
void m2_Write(char ch) { putchar(ch); }
void m2_Read(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; m2_InOut_Done = (c != EOF); }
void m2_ReadString(char *s) { m2_InOut_Done = (scanf("%s", s) == 1); }
void m2_ReadInt(int32_t *n) { m2_InOut_Done = (scanf("%d", n) == 1); }
void m2_ReadCard(uint32_t *n) { m2_InOut_Done = (scanf("%u", n) == 1); }

static FILE *m2_InFile = NULL;
static FILE *m2_OutFile = NULL;
void m2_OpenInput(const char *ext) {
    char name[256];
    printf("Input file: "); scanf("%255s", name);
    if (ext && ext[0]) { strcat(name, "."); strcat(name, ext); }
    m2_InFile = fopen(name, "r");
    m2_InOut_Done = (m2_InFile != NULL);
}
void m2_OpenOutput(const char *ext) {
    char name[256];
    printf("Output file: "); scanf("%255s", name);
    if (ext && ext[0]) { strcat(name, "."); strcat(name, ext); }
    m2_OutFile = fopen(name, "w");
    m2_InOut_Done = (m2_OutFile != NULL);
}
void m2_CloseInput(void) { if (m2_InFile) { fclose(m2_InFile); m2_InFile = NULL; } }
void m2_CloseOutput(void) { if (m2_OutFile) { fclose(m2_OutFile); m2_OutFile = NULL; } }

/* RealInOut module */
int m2_RealInOut_Done = 1;
void m2_ReadReal(float *r) { m2_RealInOut_Done = (scanf("%f", r) == 1); }
void m2_WriteReal(float r, int32_t w) { printf("%*g", (int)w, (double)r); }
void m2_WriteFixPt(float r, int32_t w, int32_t d) { printf("%*.*f", (int)w, (int)d, (double)r); }
void m2_WriteRealOct(float r) { printf("%.8A", (double)r); }
void m2_WriteLongReal(double r, int32_t w) { printf("%*g", (int)w, r); }
void m2_ReadLongReal(double *r) { m2_RealInOut_Done = (scanf("%lf", r) == 1); }

/* MathLib — Random/Randomize */
float m2_Random(void) { return (float)rand() / ((float)RAND_MAX + 1.0f); }
void m2_Randomize(uint32_t seed) { srand(seed); }

/* Storage module */
void m2_ALLOCATE(void **p, uint32_t size) { *p = malloc(size); }
void m2_DEALLOCATE(void **p, uint32_t size) { free(*p); *p = NULL; (void)size; }

/* Strings module */
void m2_Strings_Assign(const char *src, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(src);
    if (slen >= cap) slen = cap - 1;
    memcpy(dst, src, slen);
    dst[slen] = '\0';
}
void m2_Strings_Insert(const char *sub, char *dst, uint32_t dst_high, uint32_t pos) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(sub), dlen = strlen(dst);
    if (pos > dlen) pos = (uint32_t)dlen;
    size_t new_len = dlen + slen;
    if (new_len >= cap) new_len = cap - 1;
    if (pos + slen <= new_len) {
        memmove(dst + pos + slen, dst + pos, dlen - pos);
    }
    size_t copy_len = slen;
    if (pos + copy_len > new_len) copy_len = new_len - pos;
    memcpy(dst + pos, sub, copy_len);
    dst[new_len] = '\0';
}
void m2_Strings_Delete(char *s, uint32_t s_high, uint32_t pos, uint32_t len) {
    (void)s_high;
    size_t slen = strlen(s);
    if (pos >= slen) return;
    if (pos + len >= slen) { s[pos] = '\0'; return; }
    memmove(s + pos, s + pos + len, slen - pos - len + 1);
}
uint32_t m2_Strings_Pos(const char *sub, const char *s) {
    const char *p = strstr(s, sub);
    if (!p) return (uint32_t)-1;
    return (uint32_t)(p - s);
}
void m2_Strings_Copy(const char *src, uint32_t pos, uint32_t len, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(src);
    if (pos >= slen) { dst[0] = '\0'; return; }
    size_t avail = slen - pos;
    if (len > avail) len = (uint32_t)avail;
    if (len >= cap) len = (uint32_t)(cap - 1);
    memcpy(dst, src + pos, len);
    dst[len] = '\0';
}
void m2_Strings_Concat(const char *a, const char *b, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t alen = strlen(a), blen = strlen(b);
    size_t total = alen + blen;
    if (total >= cap) {
        if (alen >= cap) alen = cap - 1;
        blen = cap - 1 - alen;
        total = alen + blen;
    }
    memcpy(dst, a, alen);
    memcpy(dst + alen, b, blen);
    dst[total] = '\0';
}
uint32_t m2_Strings_Length(const char *s) { return (uint32_t)strlen(s); }
int32_t m2_Strings_CompareStr(const char *a, const char *b) { return (int32_t)strcmp(a, b); }

/* Args module */
uint32_t m2_Args_ArgCount(void) { return (uint32_t)m2_argc; }
void m2_Args_GetArg(uint32_t n, char *buf, uint32_t buf_high) {
    size_t cap = (size_t)buf_high + 1;
    if ((int)n < m2_argc) {
        size_t len = strlen(m2_argv[n]);
        if (len >= cap) len = cap - 1;
        memcpy(buf, m2_argv[n], len);
        buf[len] = '\0';
    } else {
        buf[0] = '\0';
    }
}

/* Terminal module (aliases to InOut) */
void m2_Terminal_Write(char ch) { putchar(ch); }
void m2_Terminal_WriteLn(void) { printf("\n"); }
void m2_Terminal_WriteString(const char *s) { printf("%s", s); }
void m2_Terminal_Read(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; }

/* ── Exception handling (SjLj — temporary parity with C backend) ──
   Target design: LLVM-native EH with invoke/landingpad/personality.
   This SjLj implementation reuses the C backend's runtime for parity. */
#include <setjmp.h>

typedef struct m2_ExcFrame {
    jmp_buf buf;
    struct m2_ExcFrame *prev;
    int exception_id;
    const char *exception_name;
    void *exception_arg;
} m2_ExcFrame;

static __thread m2_ExcFrame *m2_exc_stack = NULL;

void m2_exc_push(m2_ExcFrame *frame) {
    frame->prev = m2_exc_stack;
    frame->exception_id = 0;
    frame->exception_name = NULL;
    frame->exception_arg = NULL;
    m2_exc_stack = frame;
}

void m2_exc_pop(m2_ExcFrame *frame) {
    m2_exc_stack = frame->prev;
}

void m2_raise(int id, const char *name, void *arg) {
    if (m2_exc_stack) {
        m2_exc_stack->exception_id = id;
        m2_exc_stack->exception_name = name;
        m2_exc_stack->exception_arg = arg;
        longjmp(m2_exc_stack->buf, id ? id : 1);
    }
    fprintf(stderr, "Unhandled exception: %s (id=%d)\n", name ? name : "unknown", id);
    exit(1);
}

/* Runtime type info (for TYPECASE) */
typedef struct M2_TypeDesc {
    uint32_t   type_id;
    const char *type_name;
    struct M2_TypeDesc *parent;
    uint32_t   depth;
} M2_TypeDesc;

int32_t m2_exc_get_id(m2_ExcFrame *frame) { return frame->exception_id; }
const char *m2_exc_get_name(m2_ExcFrame *frame) { return frame->exception_name; }
void *m2_exc_get_arg(m2_ExcFrame *frame) { return frame->exception_arg; }
void m2_exc_reraise(m2_ExcFrame *frame) {
    m2_raise(frame->exception_id, frame->exception_name, frame->exception_arg);
}

/* ── LLVM-native exception handling ────────────────────────────────
   Uses the Itanium C++ ABI unwinder (_Unwind_RaiseException) with a
   custom personality function (m2_eh_personality).

   Exception object layout:
     M2UnwindException { _Unwind_Exception header; int32_t exc_id; const char *name; }

   The personality function reads the LSDA (emitted by LLVM from
   landingpad clauses) and matches exc_id against catch type info globals.

   This coexists with the SjLj runtime above — SjLj handles ISO
   module-body EXCEPT; this handles M2+ TRY/EXCEPT/FINALLY.           */

#include <unwind.h>

/* Vendor exception class: "M2\0\0\0\0\0\0" as uint64_t */
#define M2_EXCEPTION_CLASS 0x4D32000000000000ULL

typedef struct {
    _Unwind_Exception header;
    int32_t exc_id;
    const char *exc_name;
} M2UnwindException;

static void m2_unwind_cleanup(_Unwind_Reason_Code reason, _Unwind_Exception *exc) {
    free(exc);
}

/* Throw a Modula-2 exception using the LLVM/Itanium unwinder. */
void m2_eh_throw(int32_t exc_id, const char *name) {
    M2UnwindException *exc = (M2UnwindException *)malloc(sizeof(M2UnwindException));
    if (!exc) { fprintf(stderr, "m2_eh_throw: out of memory\n"); exit(1); }
    memset(&exc->header, 0, sizeof(_Unwind_Exception));
    exc->header.exception_class = M2_EXCEPTION_CLASS;
    exc->header.exception_cleanup = m2_unwind_cleanup;
    exc->exc_id = exc_id;
    exc->exc_name = name;
    _Unwind_Reason_Code rc = _Unwind_RaiseException(&exc->header);
    /* If we get here, no handler was found */
    fprintf(stderr, "Unhandled M2 exception: %s (id=%d, rc=%d)\n",
            name ? name : "unknown", exc_id, (int)rc);
    exit(1);
}

/* Extract exception ID from a landed exception pointer.
   The landingpad returns { ptr, i32 } where ptr is the _Unwind_Exception*. */
int32_t m2_eh_get_id(void *unwind_exc_ptr) {
    if (!unwind_exc_ptr) return 0;
    _Unwind_Exception *ue = (_Unwind_Exception *)unwind_exc_ptr;
    M2UnwindException *m2e = (M2UnwindException *)ue;
    return m2e->exc_id;
}

/* Begin/end catch — called to acknowledge handling. */
void *m2_eh_begin_catch(void *unwind_exc_ptr) {
    return unwind_exc_ptr; /* no-op for now — could track active exception */
}
void m2_eh_end_catch(void) {
    /* no-op for now */
}

/* Type info globals: each M2 exception ID is represented by a global i32.
   The personality function compares the landed exception's exc_id against
   these type info values. LLVM's @llvm.eh.typeid.for maps them to selectors.

   For catch-all (EXCEPT without a type), use null type info in landingpad. */

/* ── Modula-2 personality function ──────────────────────────────────
   Reads the LSDA (generated by LLVM from landingpad clauses) using
   the Itanium/DWARF encoding. Matches M2 exception IDs against
   catch type info globals.

   LSDA layout (Itanium C++ ABI, used by LLVM for all languages):
     header: lpStartEncoding, lpStart (optional), ttEncoding, ttBase (optional),
             callSiteEncoding, callSiteTableSize
     call site table: [start, len, landingPad, actionIdx]*
     action table: [typeFilter, nextAction]*
     type table: [typeInfoPtr]* (reversed, indexed from end)                    */

/* Read a ULEB128 value from *p, advance *p. */
static uintptr_t m2_read_uleb128(const uint8_t **p) {
    uintptr_t result = 0;
    unsigned shift = 0;
    uint8_t byte;
    do {
        byte = **p; (*p)++;
        result |= (uintptr_t)(byte & 0x7F) << shift;
        shift += 7;
    } while (byte & 0x80);
    return result;
}

/* Read a SLEB128 value from *p, advance *p. */
static intptr_t m2_read_sleb128(const uint8_t **p) {
    intptr_t result = 0;
    unsigned shift = 0;
    uint8_t byte;
    do {
        byte = **p; (*p)++;
        result |= (intptr_t)(byte & 0x7F) << shift;
        shift += 7;
    } while (byte & 0x80);
    if ((byte & 0x40) && (shift < sizeof(intptr_t) * 8))
        result |= -(1L << shift);
    return result;
}

/* Read an encoded pointer from *p using DW_EH_PE encoding. */
static uintptr_t m2_read_encoded(const uint8_t **p, uint8_t encoding, uintptr_t base) {
    if (encoding == 0xFF) return 0; /* DW_EH_PE_omit */
    uintptr_t result;
    const uint8_t *start = *p;
    switch (encoding & 0x0F) {
        case 0x00: /* DW_EH_PE_absptr */
            result = *(uintptr_t *)*p;
            *p += sizeof(uintptr_t);
            break;
        case 0x01: /* DW_EH_PE_uleb128 */
            result = m2_read_uleb128(p);
            break;
        case 0x02: /* DW_EH_PE_udata2 */
            result = *(uint16_t *)*p;
            *p += 2;
            break;
        case 0x03: /* DW_EH_PE_udata4 */
            result = *(uint32_t *)*p;
            *p += 4;
            break;
        case 0x04: /* DW_EH_PE_udata8 */
            result = *(uint64_t *)*p;
            *p += 8;
            break;
        case 0x09: /* DW_EH_PE_sleb128 */
            result = (uintptr_t)m2_read_sleb128(p);
            break;
        case 0x0A: /* DW_EH_PE_sdata2 */
            result = (uintptr_t)*(int16_t *)*p;
            *p += 2;
            break;
        case 0x0B: /* DW_EH_PE_sdata4 */
            result = (uintptr_t)*(int32_t *)*p;
            *p += 4;
            break;
        default:
            result = 0;
            break;
    }
    switch (encoding & 0x70) {
        case 0x00: break; /* DW_EH_PE_absptr */
        case 0x10: result += (uintptr_t)start; break; /* DW_EH_PE_pcrel */
        case 0x20: result += base; break; /* DW_EH_PE_textrel */
        case 0x30: result += base; break; /* DW_EH_PE_datarel */
        case 0x40: result += (uintptr_t)start; break; /* DW_EH_PE_funcrel */
    }
    if (encoding & 0x80) { /* DW_EH_PE_indirect */
        result = *(uintptr_t *)result;
    }
    return result;
}

_Unwind_Reason_Code m2_eh_personality(
    int version, _Unwind_Action actions,
    uint64_t exceptionClass,
    _Unwind_Exception *exceptionObject,
    struct _Unwind_Context *context)
{
    if (version != 1) return _URC_FATAL_PHASE1_ERROR;

    const uint8_t *lsda = (const uint8_t *)_Unwind_GetLanguageSpecificData(context);
    if (!lsda) return _URC_CONTINUE_UNWIND;

    uintptr_t ip = _Unwind_GetIP(context) - 1;
    uintptr_t funcStart = _Unwind_GetRegionStart(context);
    uintptr_t ipOffset = ip - funcStart;

    /* Parse LSDA header */
    const uint8_t *p = lsda;
    uint8_t lpStartEncoding = *p++;
    uintptr_t lpStart = funcStart;
    if (lpStartEncoding != 0xFF)
        lpStart = m2_read_encoded(&p, lpStartEncoding, funcStart);

    uint8_t ttEncoding = *p++;
    const uint8_t *ttBase = NULL;
    if (ttEncoding != 0xFF) {
        uintptr_t ttBaseOffset = m2_read_uleb128(&p);
        ttBase = p + ttBaseOffset;
    }

    uint8_t csEncoding = *p++;
    uintptr_t csTableSize = m2_read_uleb128(&p);
    const uint8_t *csTableEnd = p + csTableSize;
    const uint8_t *actionTable = csTableEnd;

    /* Scan call site table for matching entry */
    int found = 0;
    uintptr_t landingPad = 0;
    uintptr_t actionIdx = 0;

    while (p < csTableEnd) {
        uintptr_t csStart = m2_read_encoded(&p, csEncoding, 0);
        uintptr_t csLen   = m2_read_encoded(&p, csEncoding, 0);
        uintptr_t csLp    = m2_read_encoded(&p, csEncoding, 0);
        uintptr_t csAct   = m2_read_uleb128(&p);

        if (ipOffset >= csStart && ipOffset < csStart + csLen) {
            if (csLp) {
                landingPad = lpStart + csLp;
                actionIdx = csAct;
                found = 1;
            }
            break;
        }
    }

    if (!found) {
#ifdef M2_EH_DEBUG
        fprintf(stderr, "m2_eh_personality: no call site match for ip=%lx offset=%lx\n",
                (unsigned long)ip, (unsigned long)ipOffset);
#endif
        return _URC_CONTINUE_UNWIND;
    }
#ifdef M2_EH_DEBUG
    fprintf(stderr, "m2_eh_personality: found lp=%lx action=%lu actions=%d\n",
            (unsigned long)landingPad, (unsigned long)actionIdx, (int)actions);
#endif

    /* Determine selector value from action table */
    int32_t selector = 0;
    int matched = 0;

    if (actionIdx) {
        const uint8_t *ap = actionTable + actionIdx - 1;
        while (1) {
            intptr_t typeFilter = m2_read_sleb128(&ap);
            intptr_t nextAction = m2_read_sleb128(&ap);

#ifdef M2_EH_DEBUG
            fprintf(stderr, "  action: typeFilter=%ld ttBase=%p ttEncoding=%d\n",
                    (long)typeFilter, (void*)ttBase, (int)ttEncoding);
#endif
            if (typeFilter > 0 && ttBase && ttEncoding != 0xFF) {
                /* Positive filter: catch clause */
                /* Type info is stored in reverse order before ttBase */
                uintptr_t typeInfoSize;
                switch (ttEncoding & 0x0F) {
                    case 0x0B: typeInfoSize = 4; break;  /* sdata4 */
                    case 0x03: typeInfoSize = 4; break;  /* udata4 */
                    case 0x00: typeInfoSize = sizeof(uintptr_t); break;
                    default: typeInfoSize = 4; break;
                }
                const uint8_t *typeInfoPtr = ttBase - typeFilter * typeInfoSize;
                /* Read raw value first — if 0, it's a catch-all (null type info) */
                int32_t rawVal = *(int32_t *)typeInfoPtr;
                uintptr_t typeInfo;
                if (rawVal == 0) {
                    typeInfo = 0; /* catch-all */
                } else {
                    const uint8_t *readPtr = typeInfoPtr;
                    typeInfo = m2_read_encoded(&readPtr, ttEncoding, (uintptr_t)typeInfoPtr);
                }
#ifdef M2_EH_DEBUG
                fprintf(stderr, "  typeInfo: ptr=%p raw=%d val=%lx\n",
                        (void*)typeInfoPtr, rawVal, (unsigned long)typeInfo);
#endif

                if (typeInfo == 0) {
                    /* catch-all (null type info) */
                    selector = (int32_t)typeFilter;
                    matched = 1;
                    break;
                }
                /* typeInfo points to our M2_EXC_xxx global (an i32).
                   Compare exception ID against the global's value. */
                if (exceptionClass == M2_EXCEPTION_CLASS) {
                    M2UnwindException *m2e = (M2UnwindException *)exceptionObject;
                    int32_t *exc_type = (int32_t *)typeInfo;
                    if (m2e->exc_id == *exc_type) {
                        selector = (int32_t)typeFilter;
                        matched = 1;
                        break;
                    }
                }
            } else if (typeFilter == 0) {
                /* Cleanup (FINALLY) — always matches in phase 2 */
                if (actions & _UA_CLEANUP_PHASE) {
                    selector = 0;
                    matched = 1;
                    break;
                }
            }

            if (nextAction == 0) break;
            ap = ap + nextAction; /* relative offset, but ap already advanced */
        }
    } else {
        /* No action — cleanup landing pad */
        if (actions & _UA_CLEANUP_PHASE) {
            selector = 0;
            matched = 1;
        }
    }

    if (!matched) {
#ifdef M2_EH_DEBUG
        fprintf(stderr, "m2_eh_personality: no action match\n");
#endif
        return _URC_CONTINUE_UNWIND;
    }

#ifdef M2_EH_DEBUG
    fprintf(stderr, "m2_eh_personality: matched! selector=%d phase=%s\n",
            selector, (actions & _UA_SEARCH_PHASE) ? "search" : "cleanup");
#endif

    if (actions & _UA_SEARCH_PHASE) {
        return _URC_HANDLER_FOUND;
    }

    /* Install context for landing pad */
    _Unwind_SetGR(context, __builtin_eh_return_data_regno(0),
                  (uintptr_t)exceptionObject);
    _Unwind_SetGR(context, __builtin_eh_return_data_regno(1),
                  (uintptr_t)selector);
    _Unwind_SetIP(context, landingPad);
    return _URC_INSTALL_CONTEXT;
}

/* REF/OBJECT allocation with RTTI header */
typedef struct M2_RefHeader {
    M2_TypeDesc *td;
} M2_RefHeader;

void *M2_ref_alloc(size_t payload_size, M2_TypeDesc *td) {
    M2_RefHeader *hdr = (M2_RefHeader *)malloc(sizeof(M2_RefHeader) + payload_size);
    if (!hdr) { fprintf(stderr, "M2_ref_alloc: out of memory\n"); exit(1); }
    memset(hdr, 0, sizeof(M2_RefHeader) + payload_size);
    hdr->td = td;
    return (void *)(hdr + 1);
}

M2_TypeDesc *M2_TYPEOF(void *ref) {
    if (!ref) return NULL;
    M2_RefHeader *hdr = ((M2_RefHeader *)ref) - 1;
    return hdr->td;
}

int32_t M2_ISA(void *payload, M2_TypeDesc *target) {
    M2_TypeDesc *td = M2_TYPEOF(payload);
    if (!td || !target) return 0;
    while (td) {
        if (td == target) return 1;
        td = td->parent;
    }
    return 0;
}

void M2_ref_free(void *payload) {
    if (!payload) return;
    M2_RefHeader *hdr = ((M2_RefHeader *)payload) - 1;
    free(hdr);
}

/* ── BinaryIO module ────────────────────────────────────────── */
#define M2_MAX_FILES 32
static FILE *m2_bio_files[M2_MAX_FILES];
static int m2_bio_init = 0;
int m2_BinaryIO_Done = 1;

static void m2_bio_ensure_init(void) {
    if (!m2_bio_init) {
        for (int i = 0; i < M2_MAX_FILES; i++) m2_bio_files[i] = NULL;
        m2_bio_init = 1;
    }
}
static int m2_bio_alloc(void) {
    m2_bio_ensure_init();
    for (int i = 0; i < M2_MAX_FILES; i++) {
        if (m2_bio_files[i] == NULL) return i;
    }
    return -1;
}
void m2_BinaryIO_OpenRead(const char *name, uint32_t *fh) {
    int slot = m2_bio_alloc();
    if (slot < 0) { m2_BinaryIO_Done = 0; *fh = 0; return; }
    m2_bio_files[slot] = fopen(name, "rb");
    if (m2_bio_files[slot]) { *fh = (uint32_t)(slot + 1); m2_BinaryIO_Done = 1; }
    else { *fh = 0; m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_OpenWrite(const char *name, uint32_t *fh) {
    int slot = m2_bio_alloc();
    if (slot < 0) { m2_BinaryIO_Done = 0; *fh = 0; return; }
    m2_bio_files[slot] = fopen(name, "wb");
    if (m2_bio_files[slot]) { *fh = (uint32_t)(slot + 1); m2_BinaryIO_Done = 1; }
    else { *fh = 0; m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_Close(uint32_t fh) {
    m2_bio_ensure_init();
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fclose(m2_bio_files[fh-1]);
        m2_bio_files[fh-1] = NULL;
    }
}
void m2_BinaryIO_ReadByte(uint32_t fh, uint32_t *b) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        int c = fgetc(m2_bio_files[fh-1]);
        if (c == EOF) { *b = 0; m2_BinaryIO_Done = 0; }
        else { *b = (uint32_t)(unsigned char)c; m2_BinaryIO_Done = 1; }
    } else { *b = 0; m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_WriteByte(uint32_t fh, uint32_t b) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fputc((unsigned char)(b & 0xFF), m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_ReadBytes(uint32_t fh, char *buf, uint32_t n, uint32_t *actual) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        *actual = (uint32_t)fread(buf, 1, n, m2_bio_files[fh-1]);
        m2_BinaryIO_Done = (*actual > 0) ? 1 : 0;
    } else { *actual = 0; m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_WriteBytes(uint32_t fh, const char *buf, uint32_t n) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fwrite(buf, 1, n, m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_FileSize(uint32_t fh, uint32_t *size) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        long cur = ftell(m2_bio_files[fh-1]);
        fseek(m2_bio_files[fh-1], 0, SEEK_END);
        *size = (uint32_t)ftell(m2_bio_files[fh-1]);
        fseek(m2_bio_files[fh-1], cur, SEEK_SET);
        m2_BinaryIO_Done = 1;
    } else { *size = 0; m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_Seek(uint32_t fh, uint32_t pos) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fseek(m2_bio_files[fh-1], (long)pos, SEEK_SET);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}
void m2_BinaryIO_Tell(uint32_t fh, uint32_t *pos) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        *pos = (uint32_t)ftell(m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { *pos = 0; m2_BinaryIO_Done = 0; }
}
int m2_BinaryIO_IsEOF(uint32_t fh) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        return feof(m2_bio_files[fh-1]) ? 1 : 0;
    }
    return 1;
}
"#
    .to_string()
}

/// Map a stdlib procedure call to its C equivalent
pub fn map_stdlib_call(module: &str, proc_name: &str) -> Option<String> {
    let m = module.to_ascii_uppercase();
    let p = proc_name.to_ascii_uppercase();
    match (m.as_str(), p.as_str()) {
        // InOut
        ("INOUT", "WRITESTRING") => Some("m2_WriteString".to_string()),
        ("INOUT", "WRITELN") => Some("m2_WriteLn".to_string()),
        ("INOUT", "WRITEINT") => Some("m2_WriteInt".to_string()),
        ("INOUT", "WRITECARD") => Some("m2_WriteCard".to_string()),
        ("INOUT", "WRITEHEX") => Some("m2_WriteHex".to_string()),
        ("INOUT", "WRITEOCT") => Some("m2_WriteOct".to_string()),
        ("INOUT", "WRITE") | ("INOUT", "WRITECHAR") => Some("m2_Write".to_string()),
        ("INOUT", "READ") | ("INOUT", "READCHAR") => Some("m2_Read".to_string()),
        ("INOUT", "READSTRING") => Some("m2_ReadString".to_string()),
        ("INOUT", "READINT") => Some("m2_ReadInt".to_string()),
        ("INOUT", "READCARD") => Some("m2_ReadCard".to_string()),
        ("INOUT", "OPENINPUT") => Some("m2_OpenInput".to_string()),
        ("INOUT", "OPENOUTPUT") => Some("m2_OpenOutput".to_string()),
        ("INOUT", "CLOSEINPUT") => Some("m2_CloseInput".to_string()),
        ("INOUT", "CLOSEOUTPUT") => Some("m2_CloseOutput".to_string()),
        ("INOUT", "DONE") => Some("m2_InOut_Done".to_string()),

        // RealInOut
        ("REALINOUT", "READREAL") => Some("m2_ReadReal".to_string()),
        ("REALINOUT", "WRITEREAL") => Some("m2_WriteReal".to_string()),
        ("REALINOUT", "WRITEFIXPT") => Some("m2_WriteFixPt".to_string()),
        ("REALINOUT", "WRITEREALOCT") => Some("m2_WriteRealOct".to_string()),
        ("REALINOUT", "DONE") => Some("m2_RealInOut_Done".to_string()),

        // Storage
        ("STORAGE", "ALLOCATE") => Some("m2_ALLOCATE".to_string()),
        ("STORAGE", "DEALLOCATE") => Some("m2_DEALLOCATE".to_string()),

        // MathLib0 / MathLib
        ("MATHLIB0" | "MATHLIB", "SQRT") => Some("sqrtf".to_string()),
        ("MATHLIB0" | "MATHLIB", "SIN") => Some("sinf".to_string()),
        ("MATHLIB0" | "MATHLIB", "COS") => Some("cosf".to_string()),
        ("MATHLIB0" | "MATHLIB", "EXP") => Some("expf".to_string()),
        ("MATHLIB0" | "MATHLIB", "LN") => Some("logf".to_string()),
        ("MATHLIB0" | "MATHLIB", "ARCTAN") => Some("atanf".to_string()),
        ("MATHLIB0" | "MATHLIB", "ENTIER") => Some("(int32_t)floorf".to_string()),
        ("MATHLIB0" | "MATHLIB", "REAL") => Some("(float)".to_string()),
        ("MATHLIB0" | "MATHLIB", "RANDOM") => Some("m2_Random".to_string()),
        ("MATHLIB0" | "MATHLIB", "RANDOMIZE") => Some("m2_Randomize".to_string()),

        // Strings
        ("STRINGS", "ASSIGN") => Some("m2_Strings_Assign".to_string()),
        ("STRINGS", "INSERT") => Some("m2_Strings_Insert".to_string()),
        ("STRINGS", "DELETE") => Some("m2_Strings_Delete".to_string()),
        ("STRINGS", "POS") => Some("m2_Strings_Pos".to_string()),
        ("STRINGS", "LENGTH") => Some("m2_Strings_Length".to_string()),
        ("STRINGS", "COPY") => Some("m2_Strings_Copy".to_string()),
        ("STRINGS", "CONCAT") => Some("m2_Strings_Concat".to_string()),
        ("STRINGS", "COMPARESTR") => Some("m2_Strings_CompareStr".to_string()),
        ("STRINGS", "CAPS") => Some("m2_Strings_CAPS".to_string()),

        // Terminal
        ("TERMINAL", "READ") => Some("m2_Terminal_Read".to_string()),
        ("TERMINAL", "WRITE") => Some("m2_Terminal_Write".to_string()),
        ("TERMINAL", "WRITESTRING") => Some("m2_Terminal_WriteString".to_string()),
        ("TERMINAL", "WRITELN") => Some("m2_Terminal_WriteLn".to_string()),
        ("TERMINAL", "DONE") => Some("m2_Terminal_Done".to_string()),

        // FileSystem
        ("FILESYSTEM", "LOOKUP") => Some("m2_Lookup".to_string()),
        ("FILESYSTEM", "CLOSE") => Some("m2_Close".to_string()),
        ("FILESYSTEM", "READCHAR") => Some("m2_ReadChar".to_string()),
        ("FILESYSTEM", "WRITECHAR") => Some("m2_WriteChar".to_string()),
        ("FILESYSTEM", "DONE") => Some("m2_FileSystem_Done".to_string()),

        // SYSTEM
        ("SYSTEM", "ADR") => Some("m2_ADR".to_string()),
        ("SYSTEM", "TSIZE") => Some("m2_TSIZE".to_string()),

        // ISO STextIO
        ("STEXTIO", "WRITECHAR") => Some("m2_STextIO_WriteChar".to_string()),
        ("STEXTIO", "READCHAR") => Some("m2_STextIO_ReadChar".to_string()),
        ("STEXTIO", "WRITESTRING") => Some("m2_STextIO_WriteString".to_string()),
        ("STEXTIO", "READSTRING") => Some("m2_STextIO_ReadString".to_string()),
        ("STEXTIO", "WRITELN") => Some("m2_STextIO_WriteLn".to_string()),
        ("STEXTIO", "SKIPLINE") => Some("m2_STextIO_SkipLine".to_string()),
        ("STEXTIO", "READTOKEN") => Some("m2_STextIO_ReadToken".to_string()),

        // ISO SWholeIO
        ("SWHOLEIO", "WRITEINT") => Some("m2_SWholeIO_WriteInt".to_string()),
        ("SWHOLEIO", "READINT") => Some("m2_SWholeIO_ReadInt".to_string()),
        ("SWHOLEIO", "WRITECARD") => Some("m2_SWholeIO_WriteCard".to_string()),
        ("SWHOLEIO", "READCARD") => Some("m2_SWholeIO_ReadCard".to_string()),

        // ISO SRealIO
        ("SREALIO", "WRITEFLOAT") => Some("m2_SRealIO_WriteFloat".to_string()),
        ("SREALIO", "WRITEFIXED") => Some("m2_SRealIO_WriteFixed".to_string()),
        ("SREALIO", "WRITEREAL") => Some("m2_SRealIO_WriteReal".to_string()),
        ("SREALIO", "READREAL") => Some("m2_SRealIO_ReadReal".to_string()),

        // ISO SLongIO
        ("SLONGIO", "WRITEFLOAT") => Some("m2_SLongIO_WriteFloat".to_string()),
        ("SLONGIO", "WRITEFIXED") => Some("m2_SLongIO_WriteFixed".to_string()),
        ("SLONGIO", "WRITELONGREAL") => Some("m2_SLongIO_WriteLongReal".to_string()),
        ("SLONGIO", "READLONGREAL") => Some("m2_SLongIO_ReadLongReal".to_string()),

        // Args
        ("ARGS", "ARGCOUNT") => Some("m2_Args_ArgCount".to_string()),
        ("ARGS", "GETARG") => Some("m2_Args_GetArg".to_string()),

        // BinaryIO
        ("BINARYIO", "OPENREAD") => Some("m2_BinaryIO_OpenRead".to_string()),
        ("BINARYIO", "OPENWRITE") => Some("m2_BinaryIO_OpenWrite".to_string()),
        ("BINARYIO", "CLOSE") => Some("m2_BinaryIO_Close".to_string()),
        ("BINARYIO", "READBYTE") => Some("m2_BinaryIO_ReadByte".to_string()),
        ("BINARYIO", "WRITEBYTE") => Some("m2_BinaryIO_WriteByte".to_string()),
        ("BINARYIO", "READBYTES") => Some("m2_BinaryIO_ReadBytes".to_string()),
        ("BINARYIO", "WRITEBYTES") => Some("m2_BinaryIO_WriteBytes".to_string()),
        ("BINARYIO", "FILESIZE") => Some("m2_BinaryIO_FileSize".to_string()),
        ("BINARYIO", "SEEK") => Some("m2_BinaryIO_Seek".to_string()),
        ("BINARYIO", "TELL") => Some("m2_BinaryIO_Tell".to_string()),
        ("BINARYIO", "ISEOF") => Some("m2_BinaryIO_IsEOF".to_string()),
        ("BINARYIO", "DONE") => Some("m2_BinaryIO_Done".to_string()),

        // Thread module
        ("THREAD", "FORK") => Some("m2_Thread_Fork".to_string()),
        ("THREAD", "JOIN") => Some("m2_Thread_Join".to_string()),
        ("THREAD", "SELF") => Some("m2_Thread_Self".to_string()),
        ("THREAD", "ALERT") => Some("m2_Thread_Alert".to_string()),
        ("THREAD", "TESTALERT") => Some("m2_Thread_TestAlert".to_string()),

        // Mutex module
        ("MUTEX", "NEW") => Some("m2_Mutex_New".to_string()),
        ("MUTEX", "LOCK") => Some("m2_Mutex_Lock".to_string()),
        ("MUTEX", "UNLOCK") => Some("m2_Mutex_Unlock".to_string()),
        ("MUTEX", "FREE") => Some("m2_Mutex_Free".to_string()),

        // Condition module
        ("CONDITION", "NEW") => Some("m2_Condition_New".to_string()),
        ("CONDITION", "WAIT") => Some("m2_Condition_Wait".to_string()),
        ("CONDITION", "SIGNAL") => Some("m2_Condition_Signal".to_string()),
        ("CONDITION", "BROADCAST") => Some("m2_Condition_Broadcast".to_string()),
        ("CONDITION", "FREE") => Some("m2_Condition_Free".to_string()),

        _ => None,
    }
}

/// Get the list of exported procedure names for a stdlib module.
/// Used by the LLVM backend to declare all functions when a module is
/// imported as a whole (IMPORT InOut) rather than selectively.
pub fn get_stdlib_exports(module: &str) -> Vec<&'static str> {
    let m = module.to_ascii_uppercase();
    match m.as_str() {
        "INOUT" => vec!["WriteString", "WriteLn", "WriteInt", "WriteCard",
            "WriteHex", "WriteOct", "Write", "Read", "ReadString",
            "ReadInt", "ReadCard", "OpenInput", "OpenOutput",
            "CloseInput", "CloseOutput", "Done"],
        "REALINOUT" => vec!["ReadReal", "WriteReal", "WriteFixPt",
            "WriteRealOct", "Done"],
        "STORAGE" => vec!["ALLOCATE", "DEALLOCATE"],
        "MATHLIB0" | "MATHLIB" => vec!["sqrt", "sin", "cos", "exp", "ln",
            "arctan", "entier", "Random", "Randomize"],
        "STRINGS" => vec!["Assign", "Insert", "Delete", "Pos", "Length",
            "Copy", "Concat", "CompareStr", "CAPS"],
        "TERMINAL" => vec!["Read", "Write", "WriteString", "WriteLn", "Done"],
        "FILESYSTEM" => vec!["Lookup", "Close", "ReadChar", "WriteChar", "Done"],
        "STEXTIO" => vec!["WriteChar", "ReadChar", "WriteString", "ReadString",
            "WriteLn", "SkipLine", "ReadToken"],
        "SWHOLEIO" => vec!["WriteInt", "ReadInt", "WriteCard", "ReadCard"],
        "SREALIO" => vec!["WriteFloat", "WriteFixed", "WriteReal", "ReadReal"],
        "SLONGIO" => vec!["WriteFloat", "WriteFixed", "WriteLongReal", "ReadLongReal"],
        "ARGS" => vec!["ArgCount", "GetArg"],
        "BINARYIO" => vec!["OpenRead", "OpenWrite", "Close", "ReadByte",
            "WriteByte", "ReadBytes", "WriteBytes", "FileSize", "Seek",
            "Tell", "IsEOF", "Done"],
        "THREAD" => vec!["Fork", "Join", "Self", "Alert", "TestAlert"],
        "MUTEX" => vec!["New", "Lock", "Unlock", "Free"],
        "CONDITION" => vec!["New", "Wait", "Signal", "Broadcast", "Free"],
        _ => vec![],
    }
}

/// Check if a module name is a standard library module (handled by runtime header)
pub fn is_stdlib_module(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "INOUT"
            | "REALINOUT"
            | "STORAGE"
            | "MATHLIB0"
            | "MATHLIB"
            | "STRINGS"
            | "TERMINAL"
            | "FILESYSTEM"
            | "SYSTEM"
            | "STEXTIO"
            | "SWHOLEIO"
            | "SREALIO"
            | "SLONGIO"
            | "SIORESULT"
            | "ARGS"
            | "BINARYIO"
            | "THREAD"
            | "MUTEX"
            | "CONDITION"
    )
}

/// Return the list of all standard library module names
pub fn stdlib_module_names() -> Vec<&'static str> {
    vec![
        "InOut", "RealInOut", "MathLib", "MathLib0", "Strings", "Storage",
        "SYSTEM", "Terminal", "FileSystem", "STextIO", "SWholeIO", "SRealIO",
        "SLongIO", "SIOResult", "Args", "BinaryIO", "Thread", "Mutex", "Condition",
    ]
}

/// Return all stdlib procedure/variable documentation as (module, name, signature, doc) tuples.
/// Used by the docs panel to expose individual procedure entries.
pub fn stdlib_all_proc_docs() -> Vec<(String, String, String, String)> {
    let mut symtab = SymbolTable::new();
    let mut types = TypeRegistry::new();
    let mut results = Vec::new();

    for module_name in stdlib_module_names() {
        let scope = symtab.push_scope(module_name);
        register_module(&mut symtab, &mut types, scope, module_name);
        symtab.pop_scope();
        for sym in symtab.symbols_in_scope(scope) {
            let doc = match &sym.doc {
                Some(d) => d.clone(),
                None => continue,
            };
            let sig = match &sym.kind {
                SymbolKind::Procedure { params, return_type, .. } => {
                    let mut s = format!("PROCEDURE {}(", sym.name);
                    for (i, p) in params.iter().enumerate() {
                        if i > 0 { s.push_str("; "); }
                        if p.is_var { s.push_str("VAR "); }
                        s.push_str(&p.name);
                        s.push_str(": ");
                        s.push_str(&crate::analyze::type_to_string(&types, p.typ));
                    }
                    s.push(')');
                    if let Some(rt) = return_type {
                        s.push_str(": ");
                        s.push_str(&crate::analyze::type_to_string(&types, *rt));
                    }
                    s
                }
                SymbolKind::Variable => {
                    format!("VAR {}: {}", sym.name, crate::analyze::type_to_string(&types, sym.typ))
                }
                SymbolKind::Type => {
                    format!("TYPE {}", sym.name)
                }
                _ => continue,
            };
            results.push((module_name.to_string(), sym.name.clone(), sig, doc));
        }
    }
    results
}

/// Parameter descriptor for stdlib procedure codegen: (name, is_var, is_char, is_open_array)
pub type StdlibParam = (String, bool, bool, bool);

/// Return parameter info for a stdlib procedure, for use by codegen to register proc_params.
/// Returns None if the procedure is unknown.
pub fn get_stdlib_proc_params(module: &str, proc_name: &str) -> Option<Vec<StdlibParam>> {
    let sp = |name: &str, is_var: bool, is_char: bool| -> StdlibParam {
        (name.to_string(), is_var, is_char, false)
    };
    let m = module.to_ascii_uppercase();
    let p = proc_name.to_ascii_uppercase();
    match (m.as_str(), p.as_str()) {
        // InOut
        ("INOUT", "READ") | ("INOUT", "READCHAR") => Some(vec![sp("ch", true, true)]),
        ("INOUT", "READSTRING") => Some(vec![sp("s", true, false)]),
        ("INOUT", "READINT") => Some(vec![sp("n", true, false)]),
        ("INOUT", "READCARD") => Some(vec![sp("n", true, false)]),
        ("INOUT", "WRITE") | ("INOUT", "WRITECHAR") => Some(vec![sp("ch", false, true)]),
        ("INOUT", "WRITESTRING") => Some(vec![sp("s", false, false)]),
        ("INOUT", "WRITEINT") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("INOUT", "WRITECARD") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("INOUT", "WRITEHEX") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("INOUT", "WRITEOCT") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("INOUT", "WRITELN") => Some(vec![]),
        ("INOUT", "OPENINPUT") => Some(vec![sp("ext", false, false)]),
        ("INOUT", "OPENOUTPUT") => Some(vec![sp("ext", false, false)]),
        ("INOUT", "CLOSEINPUT") => Some(vec![]),
        ("INOUT", "CLOSEOUTPUT") => Some(vec![]),

        // Terminal
        ("TERMINAL", "READ") => Some(vec![sp("ch", true, true)]),
        ("TERMINAL", "WRITE") => Some(vec![sp("ch", false, true)]),
        ("TERMINAL", "WRITESTRING") => Some(vec![sp("s", false, false)]),
        ("TERMINAL", "WRITELN") => Some(vec![]),

        // STextIO
        ("STEXTIO", "WRITECHAR") => Some(vec![sp("ch", false, true)]),
        ("STEXTIO", "READCHAR") => Some(vec![sp("ch", true, true)]),
        ("STEXTIO", "WRITESTRING") => Some(vec![sp("s", false, false)]),
        ("STEXTIO", "READSTRING") => Some(vec![sp("s", true, false)]),
        ("STEXTIO", "WRITELN") => Some(vec![]),
        ("STEXTIO", "SKIPLINE") => Some(vec![]),
        ("STEXTIO", "READTOKEN") => Some(vec![sp("s", true, false)]),

        // SWholeIO
        ("SWHOLEIO", "WRITEINT") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("SWHOLEIO", "READINT") => Some(vec![sp("n", true, false)]),
        ("SWHOLEIO", "WRITECARD") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("SWHOLEIO", "READCARD") => Some(vec![sp("n", true, false)]),

        // SRealIO
        ("SREALIO", "WRITEFLOAT") => Some(vec![sp("r", false, false), sp("sigFigs", false, false), sp("w", false, false)]),
        ("SREALIO", "WRITEFIXED") => Some(vec![sp("r", false, false), sp("place", false, false), sp("w", false, false)]),
        ("SREALIO", "WRITEREAL") => Some(vec![sp("r", false, false), sp("w", false, false)]),
        ("SREALIO", "READREAL") => Some(vec![sp("r", true, false)]),

        // SLongIO
        ("SLONGIO", "WRITELONGFLOAT") => Some(vec![sp("r", false, false), sp("sigFigs", false, false), sp("w", false, false)]),
        ("SLONGIO", "WRITELONGFIXED") => Some(vec![sp("r", false, false), sp("place", false, false), sp("w", false, false)]),
        ("SLONGIO", "WRITELONGREAL") => Some(vec![sp("r", false, false), sp("w", false, false)]),
        ("SLONGIO", "READLONGREAL") => Some(vec![sp("r", true, false)]),

        // RealInOut
        ("REALINOUT", "READREAL") => Some(vec![sp("r", true, false)]),
        ("REALINOUT", "WRITEREAL") => Some(vec![sp("r", false, false), sp("w", false, false)]),
        ("REALINOUT", "WRITEFIXPT") => Some(vec![sp("r", false, false), sp("w", false, false), sp("d", false, false)]),
        ("REALINOUT", "WRITEREALOCT") => Some(vec![sp("r", false, false)]),

        // Storage
        ("STORAGE", "ALLOCATE") => Some(vec![sp("p", true, false), sp("size", false, false)]),
        ("STORAGE", "DEALLOCATE") => Some(vec![sp("p", true, false), sp("size", false, false)]),

        // MathLib0/MathLib - all single param returning real
        ("MATHLIB0" | "MATHLIB", "SQRT" | "SIN" | "COS" | "ARCTAN" | "EXP" | "LN") => Some(vec![sp("x", false, false)]),
        ("MATHLIB0" | "MATHLIB", "ENTIER") => Some(vec![sp("x", false, false)]),
        ("MATHLIB0" | "MATHLIB", "REAL") => Some(vec![sp("x", false, false)]),
        ("MATHLIB0" | "MATHLIB", "RANDOM") => Some(vec![]),
        ("MATHLIB0" | "MATHLIB", "RANDOMIZE") => Some(vec![sp("seed", false, false)]),

        // Strings — destination params are is_open_array so codegen emits HIGH bound
        ("STRINGS", "ASSIGN") => Some(vec![sp("src", false, false), ("dst".to_string(), false, false, true)]),
        ("STRINGS", "INSERT") => Some(vec![sp("sub", false, false), ("dst".to_string(), false, false, true), sp("pos", false, false)]),
        ("STRINGS", "DELETE") => Some(vec![("s".to_string(), false, false, true), sp("pos", false, false), sp("len", false, false)]),
        ("STRINGS", "POS") => Some(vec![sp("sub", false, false), sp("s", false, false)]),
        ("STRINGS", "LENGTH") => Some(vec![sp("s", false, false)]),
        ("STRINGS", "CAPS") => Some(vec![("s".to_string(), false, false, true)]),
        ("STRINGS", "COPY") => Some(vec![sp("src", false, false), sp("pos", false, false), sp("len", false, false), ("dst".to_string(), false, false, true)]),
        ("STRINGS", "CONCAT") => Some(vec![sp("s1", false, false), sp("s2", false, false), ("dst".to_string(), false, false, true)]),
        ("STRINGS", "COMPARESTR") => Some(vec![sp("s1", false, false), sp("s2", false, false)]),

        // FileSystem
        ("FILESYSTEM", "LOOKUP") => Some(vec![sp("f", true, false), sp("name", false, false), sp("new", false, false)]),
        ("FILESYSTEM", "CLOSE") => Some(vec![sp("f", true, false)]),
        ("FILESYSTEM", "READCHAR") => Some(vec![sp("f", true, false), sp("ch", true, true)]),
        ("FILESYSTEM", "WRITECHAR") => Some(vec![sp("f", true, false), sp("ch", false, true)]),

        // SYSTEM
        ("SYSTEM", "ADR") => Some(vec![sp("x", false, false)]),
        ("SYSTEM", "TSIZE") => Some(vec![sp("T", false, false)]),
        ("SYSTEM", "NEWPROCESS") => Some(vec![sp("p", false, false), sp("a", false, false), sp("n", false, false), sp("new", true, false)]),
        ("SYSTEM", "TRANSFER") => Some(vec![sp("from", true, false), sp("to", true, false)]),
        ("SYSTEM", "IOTRANSFER") => Some(vec![sp("from", true, false), sp("to", true, false), sp("vec", false, false)]),

        // Args
        ("ARGS", "ARGCOUNT") => Some(vec![]),
        ("ARGS", "GETARG") => Some(vec![sp("n", false, false), ("buf".to_string(), true, false, true)]),

        // BinaryIO
        ("BINARYIO", "OPENREAD") => Some(vec![sp("name", false, false), sp("fh", true, false)]),
        ("BINARYIO", "OPENWRITE") => Some(vec![sp("name", false, false), sp("fh", true, false)]),
        ("BINARYIO", "CLOSE") => Some(vec![sp("fh", false, false)]),
        ("BINARYIO", "READBYTE") => Some(vec![sp("fh", false, false), sp("b", true, false)]),
        ("BINARYIO", "WRITEBYTE") => Some(vec![sp("fh", false, false), sp("b", false, false)]),
        ("BINARYIO", "READBYTES") => Some(vec![sp("fh", false, false), sp("buf", true, false), sp("n", false, false), sp("actual", true, false)]),
        ("BINARYIO", "WRITEBYTES") => Some(vec![sp("fh", false, false), sp("buf", false, false), sp("n", false, false)]),
        ("BINARYIO", "FILESIZE") => Some(vec![sp("fh", false, false), sp("size", true, false)]),
        ("BINARYIO", "SEEK") => Some(vec![sp("fh", false, false), sp("pos", false, false)]),
        ("BINARYIO", "TELL") => Some(vec![sp("fh", false, false), sp("pos", true, false)]),
        ("BINARYIO", "ISEOF") => Some(vec![sp("fh", false, false)]),

        // Thread module
        ("THREAD", "FORK") => Some(vec![sp("p", false, false)]),
        ("THREAD", "JOIN") => Some(vec![sp("t", false, false)]),
        ("THREAD", "SELF") => Some(vec![]),
        ("THREAD", "ALERT") => Some(vec![sp("t", false, false)]),
        ("THREAD", "TESTALERT") => Some(vec![]),

        // Mutex module
        ("MUTEX", "NEW") => Some(vec![]),
        ("MUTEX", "LOCK") => Some(vec![sp("m", false, false)]),
        ("MUTEX", "UNLOCK") => Some(vec![sp("m", false, false)]),
        ("MUTEX", "FREE") => Some(vec![sp("m", false, false)]),

        // Condition module
        ("CONDITION", "NEW") => Some(vec![]),
        ("CONDITION", "WAIT") => Some(vec![sp("c", false, false), sp("m", false, false)]),
        ("CONDITION", "SIGNAL") => Some(vec![sp("c", false, false)]),
        ("CONDITION", "BROADCAST") => Some(vec![sp("c", false, false)]),
        ("CONDITION", "FREE") => Some(vec![sp("c", false, false)]),

        _ => None,
    }
}
