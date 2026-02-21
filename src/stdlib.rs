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
typedef struct m2_TypeInfo {
    int type_id;
    const char *type_name;
    struct m2_TypeInfo *parent;
} m2_TypeInfo;

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
#ifdef M2_USE_GC
#include <gc/gc.h>
#else
/* Fallback: use malloc when GC is not available */
#define GC_MALLOC(sz) malloc(sz)
#define GC_REALLOC(p, sz) realloc(p, sz)
#define GC_FREE(p) free(p)
static inline void GC_INIT(void) {}
#endif

/* Allocate a GC-traced REF object with a type tag header */
static inline void *m2_ref_alloc(size_t size, m2_TypeInfo *type_info) {
    void **block = (void **)GC_MALLOC(sizeof(void *) + size);
    block[0] = type_info; /* store type tag at offset 0 */
    return &block[1];     /* return pointer past the tag */
}

/* Retrieve the type info from a REF/REFANY pointer */
static inline m2_TypeInfo *m2_ref_typeinfo(void *ref) {
    void **block = (void **)ref;
    return (m2_TypeInfo *)block[-1];
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

/* Storage module */
static void m2_ALLOCATE(void **p, uint32_t size) { *p = malloc(size); }
static void m2_DEALLOCATE(void **p, uint32_t size) { free(*p); *p = NULL; (void)size; }

/* Strings module — bounded, always NUL-terminates, truncates on overflow */
static void m2_Strings_Assign(const char *src, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(src);
    if (slen >= cap) slen = cap - 1;
    memcpy(dst, src, slen);
    dst[slen] = '\0';
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
static void m2_Args_GetArg(uint32_t n, char *buf) {
    if ((int)n < m2_argc) {
        strcpy(buf, m2_argv[n]);
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

        // Strings
        ("STRINGS", "ASSIGN") => Some("m2_Strings_Assign".to_string()),
        ("STRINGS", "INSERT") => Some("m2_Strings_Insert".to_string()),
        ("STRINGS", "DELETE") => Some("m2_Strings_Delete".to_string()),
        ("STRINGS", "POS") => Some("m2_Strings_Pos".to_string()),
        ("STRINGS", "LENGTH") => Some("m2_Strings_Length".to_string()),
        ("STRINGS", "COPY") => Some("m2_Strings_Copy".to_string()),
        ("STRINGS", "CONCAT") => Some("m2_Strings_Concat".to_string()),
        ("STRINGS", "COMPARESTR") => Some("m2_Strings_CompareStr".to_string()),

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

        // Strings — destination params are is_open_array so codegen emits HIGH bound
        ("STRINGS", "ASSIGN") => Some(vec![sp("src", false, false), ("dst".to_string(), false, false, true)]),
        ("STRINGS", "INSERT") => Some(vec![sp("sub", false, false), ("dst".to_string(), false, false, true), sp("pos", false, false)]),
        ("STRINGS", "DELETE") => Some(vec![("s".to_string(), false, false, true), sp("pos", false, false), sp("len", false, false)]),
        ("STRINGS", "POS") => Some(vec![sp("sub", false, false), sp("s", false, false)]),
        ("STRINGS", "LENGTH") => Some(vec![sp("s", false, false)]),
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
        ("ARGS", "GETARG") => Some(vec![sp("n", false, false), sp("buf", true, false)]),

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
