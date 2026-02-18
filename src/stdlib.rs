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
    match module {
        "InOut" => register_inout(symtab, types, scope),
        "RealInOut" => register_realinout(symtab, types, scope),
        "MathLib0" | "MathLib" => register_mathlib(symtab, types, scope),
        "Strings" => register_strings(symtab, types, scope),
        "Storage" => register_storage(symtab, types, scope),
        "SYSTEM" => register_system(symtab, types, scope),
        "Terminal" => register_terminal(symtab, types, scope),
        "FileSystem" => register_filesystem(symtab, types, scope),
        // ISO standard I/O modules
        "STextIO" => register_stextio(symtab, types, scope),
        "SWholeIO" => register_swholeio(symtab, types, scope),
        "SRealIO" => register_srealio(symtab, types, scope),
        "SLongIO" => register_slongio(symtab, types, scope),
        "SIOResult" => register_sioresult(symtab, types, scope),
        "Args" => register_args(symtab, types, scope),
        "BinaryIO" => register_binaryio(symtab, types, scope),
        // Modula-2+ concurrency modules
        "Thread" => register_thread(symtab, types, scope),
        "Mutex" => register_mutex(symtab, types, scope),
        "Condition" => register_condition(symtab, types, scope),
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
        },
    );
}

fn def_var(symtab: &mut SymbolTable, scope: usize, name: &str, typ: TypeId) {
    let _ = symtab.define(
        scope,
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Variable,
            typ,
            exported: true,
            module: None,
            loc: SourceLoc::default(),
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
    def_proc(symtab, scope, "Read", vec![p("ch", TY_CHAR, true)], None);
    def_proc(symtab, scope, "ReadString", vec![p("s", TY_STRING, true)], None);
    def_proc(symtab, scope, "ReadInt", vec![p("n", TY_INTEGER, true)], None);
    def_proc(symtab, scope, "ReadCard", vec![p("n", TY_CARDINAL, true)], None);
    def_proc(symtab, scope, "Write", vec![p("ch", TY_CHAR, false)], None);
    def_proc(symtab, scope, "WriteString", vec![p("s", TY_STRING, false)], None);
    def_proc(symtab, scope, "WriteInt", vec![p("n", TY_INTEGER, false), p("w", TY_INTEGER, false)], None);
    def_proc(symtab, scope, "WriteCard", vec![p("n", TY_CARDINAL, false), p("w", TY_INTEGER, false)], None);
    def_proc(symtab, scope, "WriteHex", vec![p("n", TY_CARDINAL, false), p("w", TY_INTEGER, false)], None);
    def_proc(symtab, scope, "WriteOct", vec![p("n", TY_CARDINAL, false), p("w", TY_INTEGER, false)], None);
    def_proc(symtab, scope, "WriteLn", vec![], None);
    def_var(symtab, scope, "Done", TY_BOOLEAN);
    def_proc(symtab, scope, "OpenInput", vec![p("ext", TY_STRING, false)], None);
    def_proc(symtab, scope, "OpenOutput", vec![p("ext", TY_STRING, false)], None);
    def_proc(symtab, scope, "CloseInput", vec![], None);
    def_proc(symtab, scope, "CloseOutput", vec![], None);
}

fn register_realinout(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "ReadReal", vec![p("r", TY_REAL, true)], None);
    def_proc(symtab, scope, "WriteReal", vec![p("r", TY_REAL, false), p("w", TY_INTEGER, false)], None);
    def_proc(symtab, scope, "WriteFixPt", vec![
        p("r", TY_REAL, false),
        p("w", TY_INTEGER, false),
        p("d", TY_INTEGER, false),
    ], None);
    def_proc(symtab, scope, "WriteRealOct", vec![p("r", TY_REAL, false)], None);
    def_var(symtab, scope, "Done", TY_BOOLEAN);
}

fn register_mathlib(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "sqrt", vec![p("x", TY_REAL, false)], Some(TY_REAL));
    def_proc(symtab, scope, "sin", vec![p("x", TY_REAL, false)], Some(TY_REAL));
    def_proc(symtab, scope, "cos", vec![p("x", TY_REAL, false)], Some(TY_REAL));
    def_proc(symtab, scope, "arctan", vec![p("x", TY_REAL, false)], Some(TY_REAL));
    def_proc(symtab, scope, "exp", vec![p("x", TY_REAL, false)], Some(TY_REAL));
    def_proc(symtab, scope, "ln", vec![p("x", TY_REAL, false)], Some(TY_REAL));
    def_proc(symtab, scope, "entier", vec![p("x", TY_REAL, false)], Some(TY_INTEGER));
    def_proc(symtab, scope, "real", vec![p("x", TY_INTEGER, false)], Some(TY_REAL));
}

fn register_strings(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "Assign", vec![p("src", TY_STRING, false), p("dst", TY_STRING, true)], None);
    def_proc(symtab, scope, "Insert", vec![
        p("sub", TY_STRING, false),
        p("dst", TY_STRING, true),
        p("pos", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "Delete", vec![
        p("s", TY_STRING, true),
        p("pos", TY_CARDINAL, false),
        p("len", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "Pos", vec![
        p("sub", TY_STRING, false),
        p("s", TY_STRING, false),
    ], Some(TY_CARDINAL));
    def_proc(symtab, scope, "Length", vec![p("s", TY_STRING, false)], Some(TY_CARDINAL));
    def_proc(symtab, scope, "Copy", vec![
        p("src", TY_STRING, false),
        p("pos", TY_CARDINAL, false),
        p("len", TY_CARDINAL, false),
        p("dst", TY_STRING, true),
    ], None);
    def_proc(symtab, scope, "Concat", vec![
        p("s1", TY_STRING, false),
        p("s2", TY_STRING, false),
        p("dst", TY_STRING, true),
    ], None);
    def_proc(symtab, scope, "CompareStr", vec![
        p("s1", TY_STRING, false),
        p("s2", TY_STRING, false),
    ], Some(TY_INTEGER));
}

fn register_storage(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "ALLOCATE", vec![
        p("p", TY_ADDRESS, true),
        p("size", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "DEALLOCATE", vec![
        p("p", TY_ADDRESS, true),
        p("size", TY_CARDINAL, false),
    ], None);
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
    });
    let _ = symtab.define(scope, Symbol {
        name: "BYTE".to_string(),
        kind: SymbolKind::Type,
        typ: TY_BYTE,
        exported: true,
        module: Some("SYSTEM".to_string()),
        loc: SourceLoc::default(),
    });
    let _ = symtab.define(scope, Symbol {
        name: "ADDRESS".to_string(),
        kind: SymbolKind::Type,
        typ: TY_ADDRESS,
        exported: true,
        module: Some("SYSTEM".to_string()),
        loc: SourceLoc::default(),
    });

    // Procedures
    def_proc(symtab, scope, "ADR", vec![p("x", TY_INTEGER, false)], Some(TY_ADDRESS));
    def_proc(symtab, scope, "TSIZE", vec![p("T", TY_INTEGER, false)], Some(TY_CARDINAL));
    def_proc(symtab, scope, "NEWPROCESS", vec![
        p("p", TY_ADDRESS, false),
        p("a", TY_ADDRESS, false),
        p("n", TY_CARDINAL, false),
        p("new", TY_ADDRESS, true),
    ], None);
    def_proc(symtab, scope, "TRANSFER", vec![
        p("from", TY_ADDRESS, true),
        p("to", TY_ADDRESS, true),
    ], None);
    def_proc(symtab, scope, "IOTRANSFER", vec![
        p("from", TY_ADDRESS, true),
        p("to", TY_ADDRESS, true),
        p("vec", TY_CARDINAL, false),
    ], None);
}

fn register_terminal(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "Read", vec![p("ch", TY_CHAR, true)], None);
    def_proc(symtab, scope, "Write", vec![p("ch", TY_CHAR, false)], None);
    def_proc(symtab, scope, "WriteString", vec![p("s", TY_STRING, false)], None);
    def_proc(symtab, scope, "WriteLn", vec![], None);
    def_var(symtab, scope, "Done", TY_BOOLEAN);
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
    });

    def_proc(symtab, scope, "Lookup", vec![
        p("f", file_type, true),
        p("name", TY_STRING, false),
        p("new", TY_BOOLEAN, false),
    ], None);
    def_proc(symtab, scope, "Close", vec![p("f", file_type, true)], None);
    def_proc(symtab, scope, "ReadChar", vec![
        p("f", file_type, true),
        p("ch", TY_CHAR, true),
    ], None);
    def_proc(symtab, scope, "WriteChar", vec![
        p("f", file_type, true),
        p("ch", TY_CHAR, false),
    ], None);
    def_var(symtab, scope, "Done", TY_BOOLEAN);
}

// ── ISO Standard Library Modules ──────────────────────────────────────

fn register_stextio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "WriteChar", vec![p("ch", TY_CHAR, false)], None);
    def_proc(symtab, scope, "ReadChar", vec![p("ch", TY_CHAR, true)], None);
    def_proc(symtab, scope, "WriteString", vec![p("s", TY_STRING, false)], None);
    def_proc(symtab, scope, "ReadString", vec![p("s", TY_STRING, true)], None);
    def_proc(symtab, scope, "WriteLn", vec![], None);
    def_proc(symtab, scope, "SkipLine", vec![], None);
    def_proc(symtab, scope, "ReadToken", vec![p("s", TY_STRING, true)], None);
}

fn register_swholeio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "WriteInt", vec![p("n", TY_INTEGER, false), p("w", TY_CARDINAL, false)], None);
    def_proc(symtab, scope, "ReadInt", vec![p("n", TY_INTEGER, true)], None);
    def_proc(symtab, scope, "WriteCard", vec![p("n", TY_CARDINAL, false), p("w", TY_CARDINAL, false)], None);
    def_proc(symtab, scope, "ReadCard", vec![p("n", TY_CARDINAL, true)], None);
}

fn register_srealio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "WriteFloat", vec![
        p("r", TY_REAL, false), p("sigFigs", TY_CARDINAL, false), p("w", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "WriteFixed", vec![
        p("r", TY_REAL, false), p("place", TY_INTEGER, false), p("w", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "WriteReal", vec![p("r", TY_REAL, false), p("w", TY_CARDINAL, false)], None);
    def_proc(symtab, scope, "ReadReal", vec![p("r", TY_REAL, true)], None);
}

fn register_slongio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "WriteFloat", vec![
        p("r", TY_LONGREAL, false), p("sigFigs", TY_CARDINAL, false), p("w", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "WriteFixed", vec![
        p("r", TY_LONGREAL, false), p("place", TY_INTEGER, false), p("w", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "WriteLongReal", vec![p("r", TY_LONGREAL, false), p("w", TY_CARDINAL, false)], None);
    def_proc(symtab, scope, "ReadLongReal", vec![p("r", TY_LONGREAL, true)], None);
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
    });
    let _ = symtab.define(scope, Symbol {
        name: "allRight".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(0)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
    });
    let _ = symtab.define(scope, Symbol {
        name: "outOfRange".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(1)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
    });
    let _ = symtab.define(scope, Symbol {
        name: "wrongFormat".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(2)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
    });
    let _ = symtab.define(scope, Symbol {
        name: "endOfInput".to_string(),
        kind: SymbolKind::Constant(ConstValue::Integer(3)),
        typ: TY_INTEGER,
        exported: true,
        module: Some("SIOResult".to_string()),
        loc: SourceLoc::default(),
    });
}

// ── Args module ───────────────────────────────────────────────────────

fn register_args(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    def_proc(symtab, scope, "ArgCount", vec![], Some(TY_CARDINAL));
    def_proc(symtab, scope, "GetArg", vec![
        p("n", TY_CARDINAL, false),
        p("buf", TY_STRING, true),
    ], None);
}

// ── BinaryIO module ──────────────────────────────────────────────────

fn register_binaryio(symtab: &mut SymbolTable, _types: &mut TypeRegistry, scope: usize) {
    // FileHandle is represented as an opaque CARDINAL (actually a FILE* index)
    def_proc(symtab, scope, "OpenRead", vec![
        p("name", TY_STRING, false),
        p("fh", TY_CARDINAL, true),
    ], None);
    def_proc(symtab, scope, "OpenWrite", vec![
        p("name", TY_STRING, false),
        p("fh", TY_CARDINAL, true),
    ], None);
    def_proc(symtab, scope, "Close", vec![p("fh", TY_CARDINAL, false)], None);
    def_proc(symtab, scope, "ReadByte", vec![
        p("fh", TY_CARDINAL, false),
        p("b", TY_CARDINAL, true),
    ], None);
    def_proc(symtab, scope, "WriteByte", vec![
        p("fh", TY_CARDINAL, false),
        p("b", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "ReadBytes", vec![
        p("fh", TY_CARDINAL, false),
        p("buf", TY_STRING, true),
        p("n", TY_CARDINAL, false),
        p("actual", TY_CARDINAL, true),
    ], None);
    def_proc(symtab, scope, "WriteBytes", vec![
        p("fh", TY_CARDINAL, false),
        p("buf", TY_STRING, false),
        p("n", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "FileSize", vec![
        p("fh", TY_CARDINAL, false),
        p("size", TY_CARDINAL, true),
    ], None);
    def_proc(symtab, scope, "Seek", vec![
        p("fh", TY_CARDINAL, false),
        p("pos", TY_CARDINAL, false),
    ], None);
    def_proc(symtab, scope, "Tell", vec![
        p("fh", TY_CARDINAL, false),
        p("pos", TY_CARDINAL, true),
    ], None);
    def_proc(symtab, scope, "IsEOF", vec![
        p("fh", TY_CARDINAL, false),
    ], Some(TY_BOOLEAN));
    def_var(symtab, scope, "Done", TY_BOOLEAN);
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
    });
    // Fork(p: PROCEDURE): T
    def_proc(symtab, scope, "Fork", vec![p("p", TY_ADDRESS, false)], Some(TY_ADDRESS));
    // Join(t: T)
    def_proc(symtab, scope, "Join", vec![p("t", TY_ADDRESS, false)], None);
    // Self(): T
    def_proc(symtab, scope, "Self", vec![], Some(TY_ADDRESS));
    // Alert(t: T)
    def_proc(symtab, scope, "Alert", vec![p("t", TY_ADDRESS, false)], None);
    // TestAlert(): BOOLEAN
    def_proc(symtab, scope, "TestAlert", vec![], Some(TY_BOOLEAN));
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
    });
    def_proc(symtab, scope, "New", vec![], Some(TY_ADDRESS));
    def_proc(symtab, scope, "Lock", vec![p("m", TY_ADDRESS, false)], None);
    def_proc(symtab, scope, "Unlock", vec![p("m", TY_ADDRESS, false)], None);
    def_proc(symtab, scope, "Free", vec![p("m", TY_ADDRESS, false)], None);
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
    });
    def_proc(symtab, scope, "New", vec![], Some(TY_ADDRESS));
    def_proc(symtab, scope, "Wait", vec![p("c", TY_ADDRESS, false), p("m", TY_ADDRESS, false)], None);
    def_proc(symtab, scope, "Signal", vec![p("c", TY_ADDRESS, false)], None);
    def_proc(symtab, scope, "Broadcast", vec![p("c", TY_ADDRESS, false)], None);
    def_proc(symtab, scope, "Free", vec![p("c", TY_ADDRESS, false)], None);
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
    match (module, proc_name) {
        // InOut
        ("InOut", "WriteString") => Some("m2_WriteString".to_string()),
        ("InOut", "WriteLn") => Some("m2_WriteLn".to_string()),
        ("InOut", "WriteInt") => Some("m2_WriteInt".to_string()),
        ("InOut", "WriteCard") => Some("m2_WriteCard".to_string()),
        ("InOut", "WriteHex") => Some("m2_WriteHex".to_string()),
        ("InOut", "WriteOct") => Some("m2_WriteOct".to_string()),
        ("InOut", "Write") => Some("m2_Write".to_string()),
        ("InOut", "Read") => Some("m2_Read".to_string()),
        ("InOut", "ReadString") => Some("m2_ReadString".to_string()),
        ("InOut", "ReadInt") => Some("m2_ReadInt".to_string()),
        ("InOut", "ReadCard") => Some("m2_ReadCard".to_string()),
        ("InOut", "OpenInput") => Some("m2_OpenInput".to_string()),
        ("InOut", "OpenOutput") => Some("m2_OpenOutput".to_string()),
        ("InOut", "CloseInput") => Some("m2_CloseInput".to_string()),
        ("InOut", "CloseOutput") => Some("m2_CloseOutput".to_string()),
        ("InOut", "Done") => Some("m2_InOut_Done".to_string()),

        // RealInOut
        ("RealInOut", "ReadReal") => Some("m2_ReadReal".to_string()),
        ("RealInOut", "WriteReal") => Some("m2_WriteReal".to_string()),
        ("RealInOut", "WriteFixPt") => Some("m2_WriteFixPt".to_string()),
        ("RealInOut", "WriteRealOct") => Some("m2_WriteRealOct".to_string()),
        ("RealInOut", "Done") => Some("m2_RealInOut_Done".to_string()),

        // Storage
        ("Storage", "ALLOCATE") => Some("m2_ALLOCATE".to_string()),
        ("Storage", "DEALLOCATE") => Some("m2_DEALLOCATE".to_string()),

        // MathLib0 / MathLib
        ("MathLib0", "sqrt") | ("MathLib", "sqrt") => Some("sqrtf".to_string()),
        ("MathLib0", "sin") | ("MathLib", "sin") => Some("sinf".to_string()),
        ("MathLib0", "cos") | ("MathLib", "cos") => Some("cosf".to_string()),
        ("MathLib0", "exp") | ("MathLib", "exp") => Some("expf".to_string()),
        ("MathLib0", "ln") | ("MathLib", "ln") => Some("logf".to_string()),
        ("MathLib0", "arctan") | ("MathLib", "arctan") => Some("atanf".to_string()),
        ("MathLib0", "entier") | ("MathLib", "entier") => Some("(int32_t)floorf".to_string()),
        ("MathLib0", "real") | ("MathLib", "real") => Some("(float)".to_string()),

        // Strings
        ("Strings", "Assign") => Some("m2_Strings_Assign".to_string()),
        ("Strings", "Insert") => Some("m2_Strings_Insert".to_string()),
        ("Strings", "Delete") => Some("m2_Strings_Delete".to_string()),
        ("Strings", "Pos") => Some("m2_Strings_Pos".to_string()),
        ("Strings", "Length") => Some("m2_Strings_Length".to_string()),
        ("Strings", "Copy") => Some("m2_Strings_Copy".to_string()),
        ("Strings", "Concat") => Some("m2_Strings_Concat".to_string()),
        ("Strings", "CompareStr") => Some("m2_Strings_CompareStr".to_string()),

        // Terminal
        ("Terminal", "Read") => Some("m2_Terminal_Read".to_string()),
        ("Terminal", "Write") => Some("m2_Terminal_Write".to_string()),
        ("Terminal", "WriteString") => Some("m2_Terminal_WriteString".to_string()),
        ("Terminal", "WriteLn") => Some("m2_Terminal_WriteLn".to_string()),
        ("Terminal", "Done") => Some("m2_Terminal_Done".to_string()),

        // FileSystem
        ("FileSystem", "Lookup") => Some("m2_Lookup".to_string()),
        ("FileSystem", "Close") => Some("m2_Close".to_string()),
        ("FileSystem", "ReadChar") => Some("m2_ReadChar".to_string()),
        ("FileSystem", "WriteChar") => Some("m2_WriteChar".to_string()),
        ("FileSystem", "Done") => Some("m2_FileSystem_Done".to_string()),

        // SYSTEM
        ("SYSTEM", "ADR") => Some("m2_ADR".to_string()),
        ("SYSTEM", "TSIZE") => Some("m2_TSIZE".to_string()),

        // ISO STextIO
        ("STextIO", "WriteChar") => Some("m2_STextIO_WriteChar".to_string()),
        ("STextIO", "ReadChar") => Some("m2_STextIO_ReadChar".to_string()),
        ("STextIO", "WriteString") => Some("m2_STextIO_WriteString".to_string()),
        ("STextIO", "ReadString") => Some("m2_STextIO_ReadString".to_string()),
        ("STextIO", "WriteLn") => Some("m2_STextIO_WriteLn".to_string()),
        ("STextIO", "SkipLine") => Some("m2_STextIO_SkipLine".to_string()),
        ("STextIO", "ReadToken") => Some("m2_STextIO_ReadToken".to_string()),

        // ISO SWholeIO
        ("SWholeIO", "WriteInt") => Some("m2_SWholeIO_WriteInt".to_string()),
        ("SWholeIO", "ReadInt") => Some("m2_SWholeIO_ReadInt".to_string()),
        ("SWholeIO", "WriteCard") => Some("m2_SWholeIO_WriteCard".to_string()),
        ("SWholeIO", "ReadCard") => Some("m2_SWholeIO_ReadCard".to_string()),

        // ISO SRealIO
        ("SRealIO", "WriteFloat") => Some("m2_SRealIO_WriteFloat".to_string()),
        ("SRealIO", "WriteFixed") => Some("m2_SRealIO_WriteFixed".to_string()),
        ("SRealIO", "WriteReal") => Some("m2_SRealIO_WriteReal".to_string()),
        ("SRealIO", "ReadReal") => Some("m2_SRealIO_ReadReal".to_string()),

        // ISO SLongIO
        ("SLongIO", "WriteFloat") => Some("m2_SLongIO_WriteFloat".to_string()),
        ("SLongIO", "WriteFixed") => Some("m2_SLongIO_WriteFixed".to_string()),
        ("SLongIO", "WriteLongReal") => Some("m2_SLongIO_WriteLongReal".to_string()),
        ("SLongIO", "ReadLongReal") => Some("m2_SLongIO_ReadLongReal".to_string()),

        // Args
        ("Args", "ArgCount") => Some("m2_Args_ArgCount".to_string()),
        ("Args", "GetArg") => Some("m2_Args_GetArg".to_string()),

        // BinaryIO
        ("BinaryIO", "OpenRead") => Some("m2_BinaryIO_OpenRead".to_string()),
        ("BinaryIO", "OpenWrite") => Some("m2_BinaryIO_OpenWrite".to_string()),
        ("BinaryIO", "Close") => Some("m2_BinaryIO_Close".to_string()),
        ("BinaryIO", "ReadByte") => Some("m2_BinaryIO_ReadByte".to_string()),
        ("BinaryIO", "WriteByte") => Some("m2_BinaryIO_WriteByte".to_string()),
        ("BinaryIO", "ReadBytes") => Some("m2_BinaryIO_ReadBytes".to_string()),
        ("BinaryIO", "WriteBytes") => Some("m2_BinaryIO_WriteBytes".to_string()),
        ("BinaryIO", "FileSize") => Some("m2_BinaryIO_FileSize".to_string()),
        ("BinaryIO", "Seek") => Some("m2_BinaryIO_Seek".to_string()),
        ("BinaryIO", "Tell") => Some("m2_BinaryIO_Tell".to_string()),
        ("BinaryIO", "IsEOF") => Some("m2_BinaryIO_IsEOF".to_string()),
        ("BinaryIO", "Done") => Some("m2_BinaryIO_Done".to_string()),

        // Thread module
        ("Thread", "Fork") => Some("m2_Thread_Fork".to_string()),
        ("Thread", "Join") => Some("m2_Thread_Join".to_string()),
        ("Thread", "Self") => Some("m2_Thread_Self".to_string()),
        ("Thread", "Alert") => Some("m2_Thread_Alert".to_string()),
        ("Thread", "TestAlert") => Some("m2_Thread_TestAlert".to_string()),

        // Mutex module
        ("Mutex", "New") => Some("m2_Mutex_New".to_string()),
        ("Mutex", "Lock") => Some("m2_Mutex_Lock".to_string()),
        ("Mutex", "Unlock") => Some("m2_Mutex_Unlock".to_string()),
        ("Mutex", "Free") => Some("m2_Mutex_Free".to_string()),

        // Condition module
        ("Condition", "New") => Some("m2_Condition_New".to_string()),
        ("Condition", "Wait") => Some("m2_Condition_Wait".to_string()),
        ("Condition", "Signal") => Some("m2_Condition_Signal".to_string()),
        ("Condition", "Broadcast") => Some("m2_Condition_Broadcast".to_string()),
        ("Condition", "Free") => Some("m2_Condition_Free".to_string()),

        _ => None,
    }
}

/// Check if a module name is a standard library module (handled by runtime header)
pub fn is_stdlib_module(name: &str) -> bool {
    matches!(
        name,
        "InOut"
            | "RealInOut"
            | "Storage"
            | "MathLib0"
            | "MathLib"
            | "Strings"
            | "Terminal"
            | "FileSystem"
            | "SYSTEM"
            | "STextIO"
            | "SWholeIO"
            | "SRealIO"
            | "SLongIO"
            | "SIOResult"
            | "Args"
            | "BinaryIO"
            | "Thread"
            | "Mutex"
            | "Condition"
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

/// Parameter descriptor for stdlib procedure codegen: (name, is_var, is_char, is_open_array)
pub type StdlibParam = (String, bool, bool, bool);

/// Return parameter info for a stdlib procedure, for use by codegen to register proc_params.
/// Returns None if the procedure is unknown.
pub fn get_stdlib_proc_params(module: &str, proc_name: &str) -> Option<Vec<StdlibParam>> {
    let sp = |name: &str, is_var: bool, is_char: bool| -> StdlibParam {
        (name.to_string(), is_var, is_char, false)
    };
    match (module, proc_name) {
        // InOut
        ("InOut", "Read") => Some(vec![sp("ch", true, true)]),
        ("InOut", "ReadString") => Some(vec![sp("s", true, false)]),
        ("InOut", "ReadInt") => Some(vec![sp("n", true, false)]),
        ("InOut", "ReadCard") => Some(vec![sp("n", true, false)]),
        ("InOut", "Write") => Some(vec![sp("ch", false, true)]),
        ("InOut", "WriteString") => Some(vec![sp("s", false, false)]),
        ("InOut", "WriteInt") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("InOut", "WriteCard") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("InOut", "WriteHex") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("InOut", "WriteOct") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("InOut", "WriteLn") => Some(vec![]),
        ("InOut", "OpenInput") => Some(vec![sp("ext", false, false)]),
        ("InOut", "OpenOutput") => Some(vec![sp("ext", false, false)]),
        ("InOut", "CloseInput") => Some(vec![]),
        ("InOut", "CloseOutput") => Some(vec![]),

        // Terminal
        ("Terminal", "Read") => Some(vec![sp("ch", true, true)]),
        ("Terminal", "Write") => Some(vec![sp("ch", false, true)]),
        ("Terminal", "WriteString") => Some(vec![sp("s", false, false)]),
        ("Terminal", "WriteLn") => Some(vec![]),

        // STextIO
        ("STextIO", "WriteChar") => Some(vec![sp("ch", false, true)]),
        ("STextIO", "ReadChar") => Some(vec![sp("ch", true, true)]),
        ("STextIO", "WriteString") => Some(vec![sp("s", false, false)]),
        ("STextIO", "ReadString") => Some(vec![sp("s", true, false)]),
        ("STextIO", "WriteLn") => Some(vec![]),
        ("STextIO", "SkipLine") => Some(vec![]),
        ("STextIO", "ReadToken") => Some(vec![sp("s", true, false)]),

        // SWholeIO
        ("SWholeIO", "WriteInt") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("SWholeIO", "ReadInt") => Some(vec![sp("n", true, false)]),
        ("SWholeIO", "WriteCard") => Some(vec![sp("n", false, false), sp("w", false, false)]),
        ("SWholeIO", "ReadCard") => Some(vec![sp("n", true, false)]),

        // SRealIO
        ("SRealIO", "WriteFloat") => Some(vec![sp("r", false, false), sp("sigFigs", false, false), sp("w", false, false)]),
        ("SRealIO", "WriteFixed") => Some(vec![sp("r", false, false), sp("place", false, false), sp("w", false, false)]),
        ("SRealIO", "WriteReal") => Some(vec![sp("r", false, false), sp("w", false, false)]),
        ("SRealIO", "ReadReal") => Some(vec![sp("r", true, false)]),

        // SLongIO
        ("SLongIO", "WriteLongFloat") => Some(vec![sp("r", false, false), sp("sigFigs", false, false), sp("w", false, false)]),
        ("SLongIO", "WriteLongFixed") => Some(vec![sp("r", false, false), sp("place", false, false), sp("w", false, false)]),
        ("SLongIO", "WriteLongReal") => Some(vec![sp("r", false, false), sp("w", false, false)]),
        ("SLongIO", "ReadLongReal") => Some(vec![sp("r", true, false)]),

        // RealInOut
        ("RealInOut", "ReadReal") => Some(vec![sp("r", true, false)]),
        ("RealInOut", "WriteReal") => Some(vec![sp("r", false, false), sp("w", false, false)]),
        ("RealInOut", "WriteFixPt") => Some(vec![sp("r", false, false), sp("w", false, false), sp("d", false, false)]),
        ("RealInOut", "WriteRealOct") => Some(vec![sp("r", false, false)]),

        // Storage
        ("Storage", "ALLOCATE") => Some(vec![sp("p", true, false), sp("size", false, false)]),
        ("Storage", "DEALLOCATE") => Some(vec![sp("p", true, false), sp("size", false, false)]),

        // MathLib0/MathLib - all single param returning real
        ("MathLib0" | "MathLib", "sqrt" | "sin" | "cos" | "arctan" | "exp" | "ln") => Some(vec![sp("x", false, false)]),
        ("MathLib0" | "MathLib", "entier") => Some(vec![sp("x", false, false)]),
        ("MathLib0" | "MathLib", "real") => Some(vec![sp("x", false, false)]),

        // Strings — destination params are is_open_array so codegen emits HIGH bound
        ("Strings", "Assign") => Some(vec![sp("src", false, false), ("dst".to_string(), false, false, true)]),
        ("Strings", "Insert") => Some(vec![sp("sub", false, false), ("dst".to_string(), false, false, true), sp("pos", false, false)]),
        ("Strings", "Delete") => Some(vec![("s".to_string(), false, false, true), sp("pos", false, false), sp("len", false, false)]),
        ("Strings", "Pos") => Some(vec![sp("sub", false, false), sp("s", false, false)]),
        ("Strings", "Length") => Some(vec![sp("s", false, false)]),
        ("Strings", "Copy") => Some(vec![sp("src", false, false), sp("pos", false, false), sp("len", false, false), ("dst".to_string(), false, false, true)]),
        ("Strings", "Concat") => Some(vec![sp("s1", false, false), sp("s2", false, false), ("dst".to_string(), false, false, true)]),
        ("Strings", "CompareStr") => Some(vec![sp("s1", false, false), sp("s2", false, false)]),

        // FileSystem
        ("FileSystem", "Lookup") => Some(vec![sp("f", true, false), sp("name", false, false), sp("new", false, false)]),
        ("FileSystem", "Close") => Some(vec![sp("f", true, false)]),
        ("FileSystem", "ReadChar") => Some(vec![sp("f", true, false), sp("ch", true, true)]),
        ("FileSystem", "WriteChar") => Some(vec![sp("f", true, false), sp("ch", false, true)]),

        // SYSTEM
        ("SYSTEM", "ADR") => Some(vec![sp("x", false, false)]),
        ("SYSTEM", "TSIZE") => Some(vec![sp("T", false, false)]),
        ("SYSTEM", "NEWPROCESS") => Some(vec![sp("p", false, false), sp("a", false, false), sp("n", false, false), sp("new", true, false)]),
        ("SYSTEM", "TRANSFER") => Some(vec![sp("from", true, false), sp("to", true, false)]),
        ("SYSTEM", "IOTRANSFER") => Some(vec![sp("from", true, false), sp("to", true, false), sp("vec", false, false)]),

        // Args
        ("Args", "ArgCount") => Some(vec![]),
        ("Args", "GetArg") => Some(vec![sp("n", false, false), sp("buf", true, false)]),

        // BinaryIO
        ("BinaryIO", "OpenRead") => Some(vec![sp("name", false, false), sp("fh", true, false)]),
        ("BinaryIO", "OpenWrite") => Some(vec![sp("name", false, false), sp("fh", true, false)]),
        ("BinaryIO", "Close") => Some(vec![sp("fh", false, false)]),
        ("BinaryIO", "ReadByte") => Some(vec![sp("fh", false, false), sp("b", true, false)]),
        ("BinaryIO", "WriteByte") => Some(vec![sp("fh", false, false), sp("b", false, false)]),
        ("BinaryIO", "ReadBytes") => Some(vec![sp("fh", false, false), sp("buf", true, false), sp("n", false, false), sp("actual", true, false)]),
        ("BinaryIO", "WriteBytes") => Some(vec![sp("fh", false, false), sp("buf", false, false), sp("n", false, false)]),
        ("BinaryIO", "FileSize") => Some(vec![sp("fh", false, false), sp("size", true, false)]),
        ("BinaryIO", "Seek") => Some(vec![sp("fh", false, false), sp("pos", false, false)]),
        ("BinaryIO", "Tell") => Some(vec![sp("fh", false, false), sp("pos", true, false)]),
        ("BinaryIO", "IsEOF") => Some(vec![sp("fh", false, false)]),

        // Thread module
        ("Thread", "Fork") => Some(vec![sp("p", false, false)]),
        ("Thread", "Join") => Some(vec![sp("t", false, false)]),
        ("Thread", "Self") => Some(vec![]),
        ("Thread", "Alert") => Some(vec![sp("t", false, false)]),
        ("Thread", "TestAlert") => Some(vec![]),

        // Mutex module
        ("Mutex", "New") => Some(vec![]),
        ("Mutex", "Lock") => Some(vec![sp("m", false, false)]),
        ("Mutex", "Unlock") => Some(vec![sp("m", false, false)]),
        ("Mutex", "Free") => Some(vec![sp("m", false, false)]),

        // Condition module
        ("Condition", "New") => Some(vec![]),
        ("Condition", "Wait") => Some(vec![sp("c", false, false), sp("m", false, false)]),
        ("Condition", "Signal") => Some(vec![sp("c", false, false)]),
        ("Condition", "Broadcast") => Some(vec![sp("c", false, false)]),
        ("Condition", "Free") => Some(vec![sp("c", false, false)]),

        _ => None,
    }
}
