use crate::errors::SourceLoc;
use crate::symtab::*;
use crate::types::*;

pub fn is_builtin_proc(name: &str) -> bool {
    matches!(
        name,
        "ABS"
            | "CAP"
            | "CHR"
            | "DEC"
            | "EXCL"
            | "FLOAT"
            | "HALT"
            | "HIGH"
            | "INC"
            | "INCL"
            | "LONG"
            | "SHORT"
            | "MAX"
            | "MIN"
            | "ODD"
            | "ORD"
            | "SIZE"
            | "TRUNC"
            | "VAL"
            | "NEW"
            | "DISPOSE"
            | "ADR"
            | "TSIZE"
            | "LFLOAT"
            | "NEWPROCESS"
            | "TRANSFER"
            | "IOTRANSFER"
            | "RE"
            | "IM"
            | "CMPLX"
            | "SHL"
            | "SHR"
            | "BAND"
            | "BOR"
            | "BXOR"
            | "BNOT"
            | "SHIFT"
            | "ROTATE"
    )
}

pub fn builtin_return_type(name: &str) -> TypeId {
    match name {
        "ABS" => TY_INTEGER,   // actually same type as arg, simplified
        "CAP" => TY_CHAR,
        "CHR" => TY_CHAR,
        "FLOAT" => TY_REAL,
        "LFLOAT" => TY_LONGREAL,
        "HIGH" => TY_CARDINAL,
        "MAX" => TY_INTEGER,
        "MIN" => TY_INTEGER,
        "ODD" => TY_BOOLEAN,
        "ORD" => TY_CARDINAL,
        "SIZE" => TY_CARDINAL,
        "TRUNC" => TY_INTEGER,
        "LONG" => TY_LONGINT,
        "SHORT" => TY_INTEGER,
        "VAL" => TY_INTEGER,   // actually depends on first arg, simplified
        "RE" => TY_REAL,
        "IM" => TY_REAL,
        "CMPLX" => TY_COMPLEX,
        "SHL" | "SHR" | "BAND" | "BOR" | "BXOR" | "BNOT" | "SHIFT" | "ROTATE" => TY_CARDINAL,
        _ => TY_VOID,          // procedures with no return
    }
}

pub fn register_builtin_types(symtab: &mut SymbolTable, _types: &TypeRegistry, scope: usize) {
    let builtins = [
        ("INTEGER", TY_INTEGER),
        ("CARDINAL", TY_CARDINAL),
        ("REAL", TY_REAL),
        ("LONGREAL", TY_LONGREAL),
        ("BOOLEAN", TY_BOOLEAN),
        ("CHAR", TY_CHAR),
        ("BITSET", TY_BITSET),
        ("LONGINT", TY_LONGINT),
        ("LONGCARD", TY_LONGCARD),
        ("COMPLEX", TY_COMPLEX),
        ("LONGCOMPLEX", TY_LONGCOMPLEX),
        ("PROC", TY_PROC),
    ];
    for (name, typ) in builtins {
        let _ = symtab.define(
            scope,
            Symbol {
                name: name.to_string(),
                kind: SymbolKind::Type,
                typ,
                exported: true,
                module: None,
                loc: SourceLoc::default(),
                doc: None,
            },
        );
    }

    // TRUE and FALSE
    let _ = symtab.define(
        scope,
        Symbol {
            name: "TRUE".to_string(),
            kind: SymbolKind::Constant(ConstValue::Boolean(true)),
            typ: TY_BOOLEAN,
            exported: true,
            module: None,
            loc: SourceLoc::default(),
            doc: None,
        },
    );
    let _ = symtab.define(
        scope,
        Symbol {
            name: "FALSE".to_string(),
            kind: SymbolKind::Constant(ConstValue::Boolean(false)),
            typ: TY_BOOLEAN,
            exported: true,
            module: None,
            loc: SourceLoc::default(),
            doc: None,
        },
    );
    let _ = symtab.define(
        scope,
        Symbol {
            name: "NIL".to_string(),
            kind: SymbolKind::Constant(ConstValue::Nil),
            typ: TY_NIL,
            exported: true,
            module: None,
            loc: SourceLoc::default(),
            doc: None,
        },
    );
}

pub fn register_builtin_procs(symtab: &mut SymbolTable, _types: &TypeRegistry, scope: usize) {
    let procs = [
        ("ABS", vec![param("x", TY_INTEGER, false)], Some(TY_INTEGER)),
        ("CAP", vec![param("ch", TY_CHAR, false)], Some(TY_CHAR)),
        ("CHR", vec![param("n", TY_CARDINAL, false)], Some(TY_CHAR)),
        ("DEC", vec![param("x", TY_INTEGER, true)], None),
        ("EXCL", vec![param("s", TY_BITSET, true), param("i", TY_INTEGER, false)], None),
        ("FLOAT", vec![param("n", TY_INTEGER, false)], Some(TY_REAL)),
        ("LFLOAT", vec![param("n", TY_INTEGER, false)], Some(TY_LONGREAL)),
        ("HALT", vec![], None),
        ("HIGH", vec![param("a", TY_INTEGER, false)], Some(TY_CARDINAL)),
        ("INC", vec![param("x", TY_INTEGER, true)], None),
        ("INCL", vec![param("s", TY_BITSET, true), param("i", TY_INTEGER, false)], None),
        ("MAX", vec![param("T", TY_INTEGER, false)], Some(TY_INTEGER)),
        ("MIN", vec![param("T", TY_INTEGER, false)], Some(TY_INTEGER)),
        ("ODD", vec![param("n", TY_INTEGER, false)], Some(TY_BOOLEAN)),
        ("ORD", vec![param("x", TY_CHAR, false)], Some(TY_CARDINAL)),
        ("SIZE", vec![param("T", TY_INTEGER, false)], Some(TY_CARDINAL)),
        ("TRUNC", vec![param("r", TY_REAL, false)], Some(TY_INTEGER)),
        ("LONG", vec![param("x", TY_INTEGER, false)], Some(TY_LONGINT)),
        ("SHORT", vec![param("x", TY_LONGINT, false)], Some(TY_INTEGER)),
        ("VAL", vec![param("T", TY_INTEGER, false), param("x", TY_INTEGER, false)], Some(TY_INTEGER)),
        ("NEW", vec![param("p", TY_ADDRESS, true)], None),
        ("DISPOSE", vec![param("p", TY_ADDRESS, true)], None),
        ("NEWPROCESS", vec![param("p", TY_ADDRESS, false), param("a", TY_ADDRESS, false), param("n", TY_CARDINAL, false), param("new", TY_ADDRESS, true)], None),
        ("TRANSFER", vec![param("from", TY_ADDRESS, true), param("to", TY_ADDRESS, true)], None),
        ("IOTRANSFER", vec![param("from", TY_ADDRESS, true), param("to", TY_ADDRESS, true), param("vec", TY_CARDINAL, false)], None),
        // ISO extensions
        ("RE", vec![param("z", TY_COMPLEX, false)], Some(TY_REAL)),
        ("IM", vec![param("z", TY_COMPLEX, false)], Some(TY_REAL)),
        ("CMPLX", vec![param("re", TY_REAL, false), param("im", TY_REAL, false)], Some(TY_COMPLEX)),
        // Bitwise operations (common extensions)
        ("SHL", vec![param("x", TY_CARDINAL, false), param("n", TY_CARDINAL, false)], Some(TY_CARDINAL)),
        ("SHR", vec![param("x", TY_CARDINAL, false), param("n", TY_CARDINAL, false)], Some(TY_CARDINAL)),
        ("BAND", vec![param("a", TY_CARDINAL, false), param("b", TY_CARDINAL, false)], Some(TY_CARDINAL)),
        ("BOR", vec![param("a", TY_CARDINAL, false), param("b", TY_CARDINAL, false)], Some(TY_CARDINAL)),
        ("BXOR", vec![param("a", TY_CARDINAL, false), param("b", TY_CARDINAL, false)], Some(TY_CARDINAL)),
        ("BNOT", vec![param("x", TY_CARDINAL, false)], Some(TY_CARDINAL)),
        // ISO SYSTEM.SHIFT / SYSTEM.ROTATE — n is signed (sign = direction)
        ("SHIFT", vec![param("val", TY_CARDINAL, false), param("n", TY_INTEGER, false)], Some(TY_CARDINAL)),
        ("ROTATE", vec![param("val", TY_CARDINAL, false), param("n", TY_INTEGER, false)], Some(TY_CARDINAL)),
    ];

    for (name, params, ret) in procs {
        let _ = symtab.define(
            scope,
            Symbol {
                name: name.to_string(),
                kind: SymbolKind::Procedure {
                    params,
                    return_type: ret,
                    is_builtin: true,
                },
                typ: TY_VOID,
                exported: true,
                module: None,
                loc: SourceLoc::default(),
                doc: None,
            },
        );
    }
}

fn param(name: &str, typ: TypeId, is_var: bool) -> ParamInfo {
    ParamInfo {
        name: name.to_string(),
        typ,
        is_var,
    }
}

/// Generate C code for a built-in procedure call
pub fn codegen_builtin(name: &str, args: &[String]) -> String {
    match name {
        "ABS" => format!("abs({})", args[0]),
        "CAP" => format!("toupper({})", args[0]),
        "CHR" => format!("((char)({}))", args[0]),
        "DEC" => {
            if args.len() > 1 {
                format!("({} -= {})", args[0], args[1])
            } else {
                format!("({}--)", args[0])
            }
        }
        "EXCL" => format!("({} &= ~(1u << ({})))", args[0], args[1]),
        "FLOAT" => format!("((float)({}))", args[0]),
        "LFLOAT" => format!("((double)({}))", args[0]),
        "HALT" => {
            if args.is_empty() {
                "exit(0)".to_string()
            } else {
                format!("exit({})", args[0])
            }
        }
        "HIGH" => format!("{}_high", args[0]),
        "INC" => {
            if args.len() > 1 {
                format!("({} += {})", args[0], args[1])
            } else {
                format!("({}++)", args[0])
            }
        }
        "INCL" => format!("({} |= (1u << ({})))", args[0], args[1]),
        "LONG" => format!("((int64_t)({}))", args[0]),
        "SHORT" => format!("((int32_t)({}))", args[0]),
        "MAX" => format!("m2_max({})", args[0]),
        "MIN" => format!("m2_min({})", args[0]),
        "ODD" => format!("(({}) & 1)", args[0]),
        "ORD" => format!("((uint32_t)((unsigned char)({})))", args[0]),
        "SIZE" => {
            // Map Modula-2 type names to C type names
            let c_type = match args[0].as_str() {
                "INTEGER" => "int32_t",
                "CARDINAL" => "uint32_t",
                "REAL" => "float",
                "LONGREAL" => "double",
                "BOOLEAN" => "int",
                "CHAR" => "char",
                "BITSET" => "uint32_t",
                "WORD" => "uint32_t",
                "BYTE" => "uint8_t",
                "ADDRESS" => "void *",
                "LONGINT" => "int64_t",
                "LONGCARD" => "uint64_t",
                other => other,
            };
            format!("sizeof({})", c_type)
        }
        "TRUNC" => format!("((int32_t)({}))", args[0]),
        "VAL" => {
            if args.len() >= 2 {
                let c_type = match args[0].as_str() {
                    "INTEGER" => "int32_t",
                    "CARDINAL" => "uint32_t",
                    "REAL" => "float",
                    "LONGREAL" => "double",
                    "BOOLEAN" => "int",
                    "CHAR" => "char",
                    "BITSET" => "uint32_t",
                    "LONGINT" => "int64_t",
                    "LONGCARD" => "uint64_t",
                    "ADDRESS" => "void *",
                    other => other,
                };
                format!("(({})({}))", c_type, args[1])
            } else {
                format!("((int32_t)({}))", args[0])
            }
        }
        "NEW" => format!("{} = GC_MALLOC(sizeof(*{}))", args[0], args[0]),
        "DISPOSE" => format!("GC_FREE({})", args[0]),
        "ADR" => format!("((void *)&({}))", args[0]),
        "NEWPROCESS" => format!("/* NEWPROCESS({}, {}, {}, {}) - coroutine not supported at runtime */\n    fprintf(stderr, \"NEWPROCESS: coroutines not supported\\n\"); exit(1)", args[0], args[1], args[2], args[3]),
        "TRANSFER" => format!("/* TRANSFER({}, {}) - coroutine not supported at runtime */\n    fprintf(stderr, \"TRANSFER: coroutines not supported\\n\"); exit(1)", args[0], args[1]),
        "IOTRANSFER" => format!("/* IOTRANSFER({}, {}, {}) - coroutine not supported at runtime */\n    fprintf(stderr, \"IOTRANSFER: coroutines not supported\\n\"); exit(1)", args[0], args[1], args[2]),
        "TSIZE" => {
            let c_type = match args[0].as_str() {
                "INTEGER" => "int32_t",
                "CARDINAL" => "uint32_t",
                "REAL" => "float",
                "LONGREAL" => "double",
                "BOOLEAN" => "int",
                "CHAR" => "char",
                "BITSET" => "uint32_t",
                "WORD" => "uint32_t",
                "BYTE" => "uint8_t",
                "ADDRESS" => "void *",
                "LONGINT" => "int64_t",
                "LONGCARD" => "uint64_t",
                other => other,
            };
            format!("((uint32_t)sizeof({}))", c_type)
        }
        "RE" => format!("({}).re", args[0]),
        "IM" => format!("({}).im", args[0]),
        "CMPLX" => {
            if args.len() >= 2 {
                format!("((m2_COMPLEX){{ .re = {}, .im = {} }})", args[0], args[1])
            } else {
                format!("((m2_COMPLEX){{ .re = {}, .im = 0.0f }})", args[0])
            }
        }
        "SHL" => format!("((uint32_t)({}) << ({}))", args[0], args[1]),
        "SHR" => format!("((uint32_t)({}) >> ({}))", args[0], args[1]),
        "BAND" => format!("((uint32_t)({}) & (uint32_t)({}))", args[0], args[1]),
        "BOR" => format!("((uint32_t)({}) | (uint32_t)({}))", args[0], args[1]),
        "BXOR" => format!("((uint32_t)({}) ^ (uint32_t)({}))", args[0], args[1]),
        "BNOT" => format!("(~(uint32_t)({}))", args[0]),
        "SHIFT" => format!("m2_shift((uint32_t)({}), ({}))", args[0], args[1]),
        "ROTATE" => format!("m2_rotate((uint32_t)({}), ({}))", args[0], args[1]),
        _ => format!("/* unknown builtin {} */", name),
    }
}
