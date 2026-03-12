use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::CompilationUnit;
use crate::codegen::CodeGen;
use crate::errors::{CompileError, CompileResult};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::identity;

/// Return the mx install prefix directory.
/// Uses MX_HOME env var if set, otherwise defaults to ~/.mx.
fn mx_home() -> Option<PathBuf> {
    std::env::var_os(identity::ENV_HOME)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(identity::HOME_DIR)))
}

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub compile_only: bool,
    pub emit_c: bool,
    pub include_paths: Vec<PathBuf>,
    pub opt_level: u8,
    pub verbose: bool,
    pub cc: String,
    /// Enable Modula-2+ extensions (concurrency, exceptions, REF, OBJECT, etc.)
    pub m2plus: bool,
    /// Extra .c/.o/.a files to pass to the C compiler for linking
    pub extra_c_files: Vec<PathBuf>,
    /// Extra -l flags for the linker
    pub link_libs: Vec<String>,
    /// Extra -L flags for the linker
    pub link_paths: Vec<String>,
    /// Emit diagnostics as JSONL to stderr instead of human-readable messages
    pub diagnostics_json: bool,
    /// Enabled feature names for conditional compilation (*$IF name*)
    pub features: Vec<String>,
    /// Raw extra flags passed to cc (from manifest [cc] cflags)
    pub extra_cflags: Vec<String>,
    /// macOS -framework flags
    pub frameworks: Vec<String>,
    /// Case-sensitive mode (default: false, Modula-2 is case-insensitive)
    pub case_sensitive: bool,
    /// Compile with debug info (-g -O0) and emit #line directives in generated C
    pub debug: bool,
    /// Emit per-module C files for multi-TU compilation
    pub emit_per_module: bool,
    /// Output directory for per-module C files (required when emit_per_module is true)
    pub out_dir: Option<PathBuf>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            compile_only: false,
            emit_c: false,
            include_paths: Vec::new(),
            opt_level: 0,
            verbose: false,
            cc: "cc".to_string(),
            m2plus: false,
            extra_c_files: Vec::new(),
            link_libs: Vec::new(),
            link_paths: Vec::new(),
            diagnostics_json: false,
            features: Vec::new(),
            extra_cflags: Vec::new(),
            frameworks: Vec::new(),
            case_sensitive: true,
            debug: false,
            emit_per_module: false,
            out_dir: None,
        }
    }
}

/// Search for a definition module (.def) file for a given module name
pub(crate) fn find_def_file(module_name: &str, input_path: &Path, include_paths: &[PathBuf]) -> Option<PathBuf> {
    // Check in the same directory as the input file
    let dir = input_path.parent().unwrap_or(Path::new("."));
    let candidates = vec![
        dir.join(format!("{}.def", module_name)),
        dir.join(format!("{}.DEF", module_name)),
        dir.join(format!("{}.def", module_name.to_lowercase())),
    ];
    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }
    // Check include paths
    for inc_dir in include_paths {
        let candidates = vec![
            inc_dir.join(format!("{}.def", module_name)),
            inc_dir.join(format!("{}.DEF", module_name)),
            inc_dir.join(format!("{}.def", module_name.to_lowercase())),
        ];
        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }
    }
    // Fallback: scan installed libraries at ~/HOME_DIR/lib/*/src/
    if let Some(home) = mx_home() {
        let lib_dir = home.join("lib");
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let src_dir = entry.path().join("src");
                if !src_dir.is_dir() {
                    continue;
                }
                let candidates = vec![
                    src_dir.join(format!("{}.def", module_name)),
                    src_dir.join(format!("{}.DEF", module_name)),
                    src_dir.join(format!("{}.def", module_name.to_lowercase())),
                ];
                for c in &candidates {
                    if c.exists() {
                        return Some(c.clone());
                    }
                }
            }
        }
    }
    None
}

/// Search for an implementation module (.mod) file for a given module name.
/// Returns all candidate paths (in priority order) so the caller can skip
/// files that don't contain the expected module type.
pub(crate) fn find_mod_file(module_name: &str, input_path: &Path, include_paths: &[PathBuf]) -> Option<PathBuf> {
    find_mod_file_candidates(module_name, input_path, include_paths).into_iter().next()
}

/// Return all candidate .mod paths for a module, in priority order.
pub(crate) fn find_mod_file_candidates(module_name: &str, input_path: &Path, include_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let dir = input_path.parent().unwrap_or(Path::new("."));
    let candidates = vec![
        dir.join(format!("{}.mod", module_name)),
        dir.join(format!("{}.MOD", module_name)),
        dir.join(format!("{}.mod", module_name.to_lowercase())),
    ];
    for c in &candidates {
        if c.exists() && !results.contains(c) {
            results.push(c.clone());
        }
    }
    for inc_dir in include_paths {
        let candidates = vec![
            inc_dir.join(format!("{}.mod", module_name)),
            inc_dir.join(format!("{}.MOD", module_name)),
            inc_dir.join(format!("{}.mod", module_name.to_lowercase())),
        ];
        for c in &candidates {
            if c.exists() && !results.contains(c) {
                results.push(c.clone());
            }
        }
    }
    // Fallback: scan installed libraries at ~/HOME_DIR/lib/*/src/
    if let Some(home) = mx_home() {
        let lib_dir = home.join("lib");
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let src_dir = entry.path().join("src");
                if !src_dir.is_dir() {
                    continue;
                }
                let candidates = vec![
                    src_dir.join(format!("{}.mod", module_name)),
                    src_dir.join(format!("{}.MOD", module_name)),
                    src_dir.join(format!("{}.mod", module_name.to_lowercase())),
                ];
                for c in &candidates {
                    if c.exists() && !results.contains(c) {
                        results.push(c.clone());
                    }
                }
            }
        }
    }
    results
}

/// Build a driver error from C compiler failure, suppressing raw C errors
/// unless MX_SHOW_C_ERRORS=1 is set.
/// Split multi-TU C output on markers and write per-module files + manifest.
///
/// Marker structure in the amalgamated C:
///   /* MX_HEADER_BEGIN */  ... runtime header ...  /* MX_HEADER_END */
///   /* MX_MODULE_BEGIN ModName */  ... types/protos ...
///   /* MX_MODULE_DEFS ModName */   ... vars/bodies/init ...
///   /* MX_MODULE_END ModName */
///   ...more modules...
///   /* MX_MAIN_BEGIN MainName */  ... main module ...  /* MX_MAIN_END */
///
/// Output files:
///   <out_dir>/_common.h   — header + all module declaration sections
///   <out_dir>/<Module>.c  — #include "_common.h" + module body section
///   <out_dir>/_main.c     — #include "_common.h" + main module section
///   <out_dir>/_manifest.txt — list of .c files
fn write_per_module_files(c_code: &str, opts: &CompileOptions) -> CompileResult<()> {
    let out_dir = opts.out_dir.clone().unwrap_or_else(|| {
        opts.input.parent().unwrap_or(Path::new(".")).join("mx_out")
    });
    fs::create_dir_all(&out_dir).map_err(|e| {
        CompileError::driver(format!("cannot create output dir '{}': {}", out_dir.display(), e))
    })?;

    // Extract the header section (runtime header)
    let header_begin = c_code.find("/* MX_HEADER_BEGIN */\n")
        .unwrap_or(0);
    let header_end_marker = "/* MX_HEADER_END */\n";
    let header_end = c_code.find(header_end_marker)
        .map(|p| p + header_end_marker.len())
        .unwrap_or(0);
    let header_section = &c_code[..header_end];

    // Parse module sections
    let mut common_header = String::from(header_section);
    let mut module_bodies: Vec<(String, String)> = Vec::new();
    let mut main_section = String::new();

    let remaining = &c_code[header_end..];

    // Scan for MODULE_BEGIN markers
    let mut search_pos = 0;
    while let Some(mb_offset) = remaining[search_pos..].find("/* MX_MODULE_BEGIN ") {
        let mb_start = search_pos + mb_offset;
        // Extract module name from marker
        let name_start = mb_start + "/* MX_MODULE_BEGIN ".len();
        let name_end = remaining[name_start..].find(" */")
            .map(|p| name_start + p)
            .unwrap_or(name_start);
        let mod_name = remaining[name_start..name_end].to_string();

        // Find MODULE_DEFS marker
        let defs_marker = format!("/* MX_MODULE_DEFS {} */\n", mod_name);
        let end_marker = format!("/* MX_MODULE_END {} */\n", mod_name);

        if let Some(defs_offset) = remaining[mb_start..].find(&defs_marker) {
            let defs_pos = mb_start + defs_offset;
            // Declaration section: from MODULE_BEGIN to MODULE_DEFS
            let begin_line_end = remaining[mb_start..].find('\n')
                .map(|p| mb_start + p + 1)
                .unwrap_or(mb_start);
            let decl_section = &remaining[begin_line_end..defs_pos];
            common_header.push_str(decl_section);

            // Body section: from MODULE_DEFS to MODULE_END
            let body_start = defs_pos + defs_marker.len();
            if let Some(end_offset) = remaining[body_start..].find(&end_marker) {
                let body_end = body_start + end_offset;
                let body_section = remaining[body_start..body_end].to_string();
                module_bodies.push((mod_name.clone(), body_section));
                search_pos = body_end + end_marker.len();
            } else {
                search_pos = defs_pos + defs_marker.len();
            }
        } else {
            search_pos = mb_start + 1;
        }
    }

    // Extract main section
    let main_begin_marker = "/* MX_MAIN_BEGIN ";
    let main_end_marker = "/* MX_MAIN_END */\n";
    if let Some(main_start) = remaining.find(main_begin_marker) {
        let content_start = remaining[main_start..].find('\n')
            .map(|p| main_start + p + 1)
            .unwrap_or(main_start);
        let content_end = remaining.find(main_end_marker)
            .unwrap_or(remaining.len());
        main_section = remaining[content_start..content_end].to_string();
    }

    // Write _common.h
    let common_path = out_dir.join("_common.h");
    fs::write(&common_path, &common_header).map_err(|e| {
        CompileError::driver(format!("cannot write '{}': {}", common_path.display(), e))
    })?;

    // Write per-module .c files
    let mut manifest_lines: Vec<String> = Vec::new();
    for (mod_name, body) in &module_bodies {
        let c_path = out_dir.join(format!("{}.c", mod_name));
        let content = format!("#include \"_common.h\"\n{}", body);
        fs::write(&c_path, &content).map_err(|e| {
            CompileError::driver(format!("cannot write '{}': {}", c_path.display(), e))
        })?;
        manifest_lines.push(format!("{}.c", mod_name));
    }

    // Write _main.c
    let main_path = out_dir.join("_main.c");
    let main_content = format!("#include \"_common.h\"\n{}", main_section);
    fs::write(&main_path, &main_content).map_err(|e| {
        CompileError::driver(format!("cannot write '{}': {}", main_path.display(), e))
    })?;
    manifest_lines.push("_main.c".to_string());

    // Write manifest
    let manifest_path = out_dir.join("_manifest.txt");
    fs::write(&manifest_path, manifest_lines.join("\n") + "\n").map_err(|e| {
        CompileError::driver(format!("cannot write '{}': {}", manifest_path.display(), e))
    })?;

    if opts.verbose {
        eprintln!("{}: wrote {} per-module files to {}", identity::COMPILER_NAME, manifest_lines.len(), out_dir.display());
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CcSeverity {
    Error,
    Warning,
    Note,
    Fatal,
}

#[derive(Debug, Clone)]
struct CcDiagnostic {
    file: String,
    line: usize,
    col: usize,
    severity: CcSeverity,
    message: String,
}

/// Parse cc stderr into structured diagnostics.
/// Handles both clang and gcc format: `file:line:col: severity: message`
fn parse_cc_stderr(stderr: &str) -> Vec<CcDiagnostic> {
    let mut diagnostics = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        // Skip caret/context lines (indented, ^, ~ markers, empty)
        if trimmed.is_empty()
            || trimmed.starts_with('^')
            || trimmed.starts_with('~')
            || trimmed.chars().all(|c| c == ' ' || c == '^' || c == '~' || c == '|')
            || (line.starts_with(' ') && !trimmed.contains(": error:") && !trimmed.contains(": warning:"))
        {
            continue;
        }

        // Find the severity marker
        let (sev_tag, severity) = if let Some(pos) = line.find(": fatal error:") {
            (pos, CcSeverity::Fatal)
        } else if let Some(pos) = line.find(": error:") {
            (pos, CcSeverity::Error)
        } else if let Some(pos) = line.find(": warning:") {
            (pos, CcSeverity::Warning)
        } else if let Some(pos) = line.find(": note:") {
            (pos, CcSeverity::Note)
        } else {
            continue;
        };

        let prefix = &line[..sev_tag];
        let msg_start = match severity {
            CcSeverity::Fatal => sev_tag + ": fatal error:".len(),
            CcSeverity::Error => sev_tag + ": error:".len(),
            CcSeverity::Warning => sev_tag + ": warning:".len(),
            CcSeverity::Note => sev_tag + ": note:".len(),
        };
        let message = line[msg_start..].trim().to_string();

        // Parse file:line:col from prefix using rsplitn to handle paths with colons
        // Format: /path/to/file.c:42:10
        let parts: Vec<&str> = prefix.rsplitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let col = match parts[0].trim().parse::<usize>() {
            Ok(c) => c,
            Err(_) => continue,
        };
        let line_num = match parts[1].trim().parse::<usize>() {
            Ok(l) => l,
            Err(_) => continue,
        };
        let file = parts[2].trim().to_string();

        diagnostics.push(CcDiagnostic {
            file,
            line: line_num,
            col,
            severity,
            message,
        });
    }
    diagnostics
}

/// Demangle a C identifier back to Modula-2 qualified name.
/// `Module_Proc` → `Module.Proc` (M2 identifiers cannot contain underscores)
fn demangle_m2_name(c_name: &str) -> String {
    c_name.replace('_', ".")
}

/// Map a C compiler error message to a Modula-2-friendly diagnostic message.
fn map_cc_message(msg: &str) -> String {
    // use of undeclared identifier 'Module_Name'
    if msg.starts_with("use of undeclared identifier '") {
        if let Some(ident) = msg.strip_prefix("use of undeclared identifier '").and_then(|s| s.strip_suffix('\'')) {
            if let Some(dot_pos) = ident.find('_') {
                let module = &ident[..dot_pos];
                let name = &ident[dot_pos + 1..];
                return format!(
                    "'{}' is not exported by module '{}', or module '{}' is not imported",
                    demangle_m2_name(name), module, module
                );
            }
            return format!("'{}' is not declared", demangle_m2_name(ident));
        }
    }

    // unknown type name 'TypeName'
    if msg.starts_with("unknown type name '") {
        if let Some(type_name) = msg.strip_prefix("unknown type name '").and_then(|s| s.strip_suffix('\'')) {
            return format!("type '{}' is not declared", demangle_m2_name(type_name));
        }
    }

    // no member named 'f' in 'struct Module_Rec'
    if msg.starts_with("no member named '") {
        if let Some(rest) = msg.strip_prefix("no member named '") {
            if let Some(tick_pos) = rest.find('\'') {
                let field = &rest[..tick_pos];
                if let Some(struct_start) = rest.find("'struct ") {
                    let after = &rest[struct_start + "'struct ".len()..];
                    if let Some(end) = after.find('\'') {
                        let struct_name = &after[..end];
                        // Extract record name from Module_Rec
                        let rec_name = if let Some(dot_pos) = struct_name.rfind('_') {
                            &struct_name[dot_pos + 1..]
                        } else {
                            struct_name
                        };
                        return format!("record type '{}' has no field '{}'", rec_name, field);
                    }
                }
            }
        }
    }

    // implicit declaration of function 'Module_P'
    if msg.starts_with("implicit declaration of function '") || msg.starts_with("call to undeclared function '") {
        let prefix = if msg.starts_with("implicit declaration") {
            "implicit declaration of function '"
        } else {
            "call to undeclared function '"
        };
        if let Some(func) = msg.strip_prefix(prefix).and_then(|s| s.strip_suffix('\'').or_else(|| s.split('\'').next())) {
            return format!("procedure '{}' is not declared", demangle_m2_name(func));
        }
    }

    // too few/many arguments
    if msg.starts_with("too few arguments") {
        return "too few arguments in procedure call".to_string();
    }
    if msg.starts_with("too many arguments") {
        return "too many arguments in procedure call".to_string();
    }

    // incompatible pointer types
    if msg.contains("incompatible pointer types") {
        return "type mismatch: incompatible types".to_string();
    }

    // redefinition of 'name'
    if msg.starts_with("redefinition of '") {
        if let Some(name) = msg.strip_prefix("redefinition of '").and_then(|s| s.strip_suffix('\'')) {
            return format!("'{}' is already defined", demangle_m2_name(name));
        }
    }

    // Unmapped: prefix with (C backend)
    format!("(C backend) {}", msg)
}

/// Add -I flags for the mx install prefix (m2sys headers, etc.)
fn add_mx_home_includes(cmd: &mut Command) {
    if let Some(home) = mx_home() {
        let m2sys_dir = home.join("lib").join("m2sys");
        if m2sys_dir.is_dir() {
            cmd.arg(format!("-I{}", m2sys_dir.display()));
        }
    }
}

/// Returns true if the file extension indicates a Modula-2 source file.
fn is_m2_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".mod") || lower.ends_with(".def")
}

/// Build a driver error from C compiler failure, mapping cc diagnostics back
/// to Modula-2 source locations via #line directives.
fn handle_cc_failure(stderr: &[u8], is_link_phase: bool, diagnostics_json: bool) -> CompileError {
    let raw = String::from_utf8_lossy(stderr);
    let show_c = std::env::var(identity::ENV_SHOW_C_ERRORS).map_or(false, |v| v == "1");

    // Linker errors have a different format — fall back to raw stderr
    if is_link_phase {
        if show_c || raw.contains("Undefined symbols") || raw.contains("ld:") {
            return CompileError::driver(format!("link failed:\n{}", raw.trim()));
        }
        return CompileError::driver(format!("link failed (internal error). Re-run with {}=1 for details.", identity::ENV_SHOW_C_ERRORS));
    }

    let cc_diags = parse_cc_stderr(&raw);

    // Filter to errors/fatals referencing M2 source files (from #line directives)
    let m2_errors: Vec<CompileError> = cc_diags
        .iter()
        .filter(|d| matches!(d.severity, CcSeverity::Error | CcSeverity::Fatal))
        .filter(|d| is_m2_file(&d.file))
        .map(|d| {
            CompileError::codegen(
                crate::errors::SourceLoc::new(&d.file, d.line, d.col),
                map_cc_message(&d.message),
            )
        })
        .collect();

    if !m2_errors.is_empty() {
        if diagnostics_json {
            emit_diagnostics_jsonl(&m2_errors);
        }
        let mut msg = m2_errors
            .iter()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join("\n");
        if show_c {
            msg.push_str(&format!("\n\n--- raw C compiler output ---\n{}", raw.trim()));
        }
        // Return the first error's location for the top-level CompileError
        return CompileError::codegen(m2_errors[0].loc.clone(), msg);
    }

    // No M2-located errors found — check for C-file errors (no #line match)
    let c_errors: Vec<CompileError> = cc_diags
        .iter()
        .filter(|d| matches!(d.severity, CcSeverity::Error | CcSeverity::Fatal))
        .map(|d| {
            CompileError::codegen(
                crate::errors::SourceLoc::new("<generated>", d.line, d.col),
                map_cc_message(&d.message),
            )
        })
        .collect();

    if !c_errors.is_empty() {
        if diagnostics_json {
            emit_diagnostics_jsonl(&c_errors);
        }
        let mut msg = c_errors
            .iter()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join("\n");
        if show_c {
            msg.push_str(&format!("\n\n--- raw C compiler output ---\n{}", raw.trim()));
        }
        return CompileError::codegen(c_errors[0].loc.clone(), msg);
    }

    // Couldn't parse any diagnostics — fall back to old behavior
    if show_c {
        CompileError::driver(format!("C backend failed:\n{}", raw.trim()))
    } else {
        CompileError::driver(
            format!("C backend failed (internal error). Re-run with {}=1 for details.", identity::ENV_SHOW_C_ERRORS)
        )
    }
}

/// Parse a source file and return the compilation unit
fn parse_file(path: &Path, case_sensitive: bool, features: &[String]) -> CompileResult<CompilationUnit> {
    let source = fs::read_to_string(path).map_err(|e| {
        CompileError::driver(format!("cannot read '{}': {}", path.display(), e))
    })?;
    let filename = path.to_string_lossy().to_string();
    let mut lexer = Lexer::new(&source, &filename);
    lexer.set_case_sensitive(case_sensitive);
    if !features.is_empty() {
        lexer.set_features(features);
    }
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_compilation_unit()
}

/// Emit a slice of CompileErrors as JSONL to stderr
fn emit_diagnostics_jsonl(errors: &[CompileError]) {
    for e in errors {
        eprintln!("{}", e.to_json());
    }
}

pub fn compile(opts: &CompileOptions) -> CompileResult<()> {
    let source = fs::read_to_string(&opts.input).map_err(|e| {
        let err = CompileError::driver(format!("cannot read '{}': {}", opts.input.display(), e));
        if opts.diagnostics_json {
            emit_diagnostics_jsonl(&[err.clone()]);
        }
        err
    })?;

    let filename = opts.input.to_string_lossy().to_string();

    if opts.verbose {
        eprintln!("{}: compiling {}", identity::COMPILER_NAME, filename);
    }

    // Lex
    let mut lexer = Lexer::new(&source, &filename);
    lexer.set_case_sensitive(opts.case_sensitive);
    if !opts.features.is_empty() {
        lexer.set_features(&opts.features);
    }
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            if opts.diagnostics_json {
                emit_diagnostics_jsonl(&[e.clone()]);
            }
            return Err(e);
        }
    };

    if opts.verbose {
        eprintln!("{}: {} tokens", identity::COMPILER_NAME, tokens.len());
    }

    // Parse
    let mut parser = Parser::new(tokens);
    let unit = match parser.parse_compilation_unit() {
        Ok(u) => u,
        Err(e) => {
            if opts.diagnostics_json {
                let accumulated = parser.get_errors();
                if accumulated.is_empty() {
                    emit_diagnostics_jsonl(&[e.clone()]);
                } else {
                    emit_diagnostics_jsonl(accumulated);
                }
            }
            return Err(e);
        }
    };

    if opts.verbose {
        eprintln!("{}: parsed successfully", identity::COMPILER_NAME);
    }

    // If this is an implementation module, look for the corresponding definition module
    let own_def = if let CompilationUnit::ImplementationModule(ref m) = unit {
        if let Some(def_path) = find_def_file(&m.name, &opts.input, &opts.include_paths) {
            if opts.verbose {
                eprintln!("{}: found definition module: {}", identity::COMPILER_NAME, def_path.display());
            }
            let def_unit = parse_file(&def_path, opts.case_sensitive, &opts.features)?;
            if let CompilationUnit::DefinitionModule(def_mod) = def_unit {
                Some(def_mod)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // For FROM Module IMPORT and IMPORT Module, find and load dependency modules
    let imports = match &unit {
        CompilationUnit::ProgramModule(m) => m.imports.clone(),
        CompilationUnit::ImplementationModule(m) => m.imports.clone(),
        _ => Vec::new(),
    };

    // Collect all imported module names (both FROM and IMPORT forms)
    let mut all_imported_modules = Vec::new();
    for imp in &imports {
        if let Some(ref from_mod) = imp.from_module {
            all_imported_modules.push(from_mod.clone());
        } else {
            for mod_name in &imp.names {
                all_imported_modules.push(mod_name.name.clone());
            }
        }
    }

    // Generate C
    let mut codegen = CodeGen::new();
    codegen.set_m2plus(opts.m2plus);
    codegen.set_debug(opts.debug);
    codegen.multi_tu = opts.emit_per_module;

    // First pass: parse all definition modules for non-stdlib imports (transitive)
    let mut registered_defs = std::collections::HashSet::new();
    // Phase 1: parse all .def files and collect them (including the own def)
    let mut parsed_defs: std::collections::HashMap<String, crate::ast::DefinitionModule> = std::collections::HashMap::new();
    // Include the own .def in parsed_defs so it participates in topological sort
    if let Some(ref def_mod) = own_def {
        // Seed queue with the own def's imports
        for imp in &def_mod.imports {
            if let Some(ref from_mod) = imp.from_module {
                all_imported_modules.push(from_mod.clone());
            } else {
                for name in &imp.names {
                    all_imported_modules.push(name.name.clone());
                }
            }
        }
        parsed_defs.insert(def_mod.name.clone(), def_mod.clone());
    }
    let mut def_queue: Vec<String> = all_imported_modules.clone();
    while let Some(mod_name) = def_queue.pop() {
        if crate::stdlib::is_stdlib_module(&mod_name) || registered_defs.contains(&mod_name) || parsed_defs.contains_key(&mod_name) {
            continue;
        }
        if let Some(def_path) = find_def_file(&mod_name, &opts.input, &opts.include_paths) {
            if opts.verbose {
                eprintln!("{}: found definition module for {}: {}", identity::COMPILER_NAME, mod_name, def_path.display());
            }
            let def_unit = parse_file(&def_path, opts.case_sensitive, &opts.features)?;
            if let CompilationUnit::DefinitionModule(def_mod) = def_unit {
                // Transitively discover imports of this def module
                for imp in &def_mod.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        if !registered_defs.contains(from_mod) && !parsed_defs.contains_key(from_mod) {
                            def_queue.push(from_mod.clone());
                        }
                    } else {
                        for name in &imp.names {
                            if !registered_defs.contains(&name.name) && !parsed_defs.contains_key(&name.name) {
                                def_queue.push(name.name.clone());
                            }
                        }
                    }
                }
                parsed_defs.insert(mod_name.clone(), def_mod);
            }
        }
    }
    // Phase 2: topologically sort def modules so dependencies are registered first
    {
        let mut sorted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        fn topo_visit(
            name: &str,
            parsed: &std::collections::HashMap<String, crate::ast::DefinitionModule>,
            visited: &mut std::collections::HashSet<String>,
            sorted: &mut Vec<String>,
            registered: &std::collections::HashSet<String>,
        ) {
            if visited.contains(name) || registered.contains(name) {
                return;
            }
            visited.insert(name.to_string());
            if let Some(def) = parsed.get(name) {
                for imp in &def.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        topo_visit(from_mod, parsed, visited, sorted, registered);
                    } else {
                        for n in &imp.names {
                            topo_visit(&n.name, parsed, visited, sorted, registered);
                        }
                    }
                }
            }
            sorted.push(name.to_string());
        }
        let names: Vec<String> = parsed_defs.keys().cloned().collect();
        for name in &names {
            topo_visit(name, &parsed_defs, &mut visited, &mut sorted, &registered_defs);
        }
        // Register in dependency order
        for name in &sorted {
            if let Some(def_mod) = parsed_defs.remove(name) {
                if opts.verbose {
                    eprintln!("{}: registering definition module: {}", identity::COMPILER_NAME, name);
                }
                codegen.register_def_module(&def_mod);
                registered_defs.insert(name.clone());
            }
        }
    }

    // Second pass: load implementation modules for all non-stdlib imported modules
    // Also transitively discover implementation module dependencies
    let mut loaded_modules = std::collections::HashSet::new();
    let mut mod_queue: Vec<String> = all_imported_modules.clone();
    while let Some(mod_name) = mod_queue.pop() {
        if crate::stdlib::is_stdlib_module(&mod_name) || loaded_modules.contains(&mod_name) {
            continue;
        }
        // Skip .mod lookup for foreign (C ABI) modules — they have no M2 implementation
        if codegen.is_foreign_module(&mod_name) {
            if opts.verbose {
                eprintln!("{}: skipping .mod lookup for foreign module {}", identity::COMPILER_NAME, mod_name);
            }
            loaded_modules.insert(mod_name.clone());
            continue;
        }
        let mod_candidates = find_mod_file_candidates(&mod_name, &opts.input, &opts.include_paths);
        let mut found_impl = None;
        for mod_path in &mod_candidates {
            if opts.verbose {
                eprintln!("{}: trying implementation module for {}: {}", identity::COMPILER_NAME, mod_name, mod_path.display());
            }
            let mod_unit = parse_file(mod_path, opts.case_sensitive, &opts.features)?;
            if let CompilationUnit::ImplementationModule(imp) = mod_unit {
                found_impl = Some(imp);
                if opts.verbose {
                    eprintln!("{}: found implementation module for {}: {}", identity::COMPILER_NAME, mod_name, mod_path.display());
                }
                break;
            }
        }
        if let Some(imp_mod) = found_impl {
            // Transitively discover dependencies of this implementation module
            for imp in &imp_mod.imports {
                if let Some(ref from_mod) = imp.from_module {
                    if !loaded_modules.contains(from_mod) {
                        // Register the def module first if not already done
                        if !registered_defs.contains(from_mod) && !crate::stdlib::is_stdlib_module(from_mod) {
                            if let Some(dep_def_path) = find_def_file(from_mod, &opts.input, &opts.include_paths) {
                                if opts.verbose {
                                    eprintln!("{}: found definition module for {}: {}", identity::COMPILER_NAME, from_mod, dep_def_path.display());
                                }
                                let dep_def_unit = parse_file(&dep_def_path, opts.case_sensitive, &opts.features)?;
                                if let CompilationUnit::DefinitionModule(dep_def) = dep_def_unit {
                                    codegen.register_def_module(&dep_def);
                                    registered_defs.insert(from_mod.clone());
                                }
                            }
                        }
                        mod_queue.push(from_mod.clone());
                    }
                } else {
                    // Whole-module import: IMPORT Module1, Module2;
                    for name in &imp.names {
                        let n = &name.name;
                        if !loaded_modules.contains(n) {
                            // Also register the def module if not already done
                            if !registered_defs.contains(n) && !crate::stdlib::is_stdlib_module(n) {
                                if let Some(dep_def_path) = find_def_file(n, &opts.input, &opts.include_paths) {
                                    if opts.verbose {
                                        eprintln!("{}: found definition module for {}: {}", identity::COMPILER_NAME, n, dep_def_path.display());
                                    }
                                    if let Ok(dep_def_unit) = parse_file(&dep_def_path, opts.case_sensitive, &opts.features) {
                                        if let CompilationUnit::DefinitionModule(dep_def) = dep_def_unit {
                                            codegen.register_def_module(&dep_def);
                                            registered_defs.insert(n.clone());
                                        }
                                    }
                                }
                            }
                            mod_queue.push(n.clone());
                        }
                    }
                }
            }
            codegen.add_imported_module(imp_mod);
            loaded_modules.insert(mod_name.clone());
        }
    }

    let c_code = if opts.diagnostics_json {
        match codegen.generate_or_errors(&unit) {
            Ok(code) => code,
            Err(errors) => {
                emit_diagnostics_jsonl(&errors);
                return Err(CompileError::codegen(
                    errors.first().map(|e| e.loc.clone()).unwrap_or_else(|| {
                        crate::errors::SourceLoc::new("<codegen>", 0, 0)
                    }),
                    errors.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join("\n"),
                ));
            }
        }
    } else {
        codegen.generate(&unit)?
    };

    if opts.verbose {
        eprintln!("{}: C code generated ({} bytes)", identity::COMPILER_NAME, c_code.len());
    }

    // Determine output paths
    let stem = opts.input.file_stem().unwrap_or_default().to_string_lossy();
    let c_file = opts
        .input
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{}.c", stem));

    if opts.emit_per_module {
        return write_per_module_files(&c_code, opts);
    }

    if opts.emit_c {
        let out_path = opts.output.clone().unwrap_or(c_file);
        fs::write(&out_path, &c_code).map_err(|e| {
            CompileError::driver(format!("cannot write '{}': {}", out_path.display(), e))
        })?;
        if opts.verbose {
            eprintln!("{}: wrote {}", identity::COMPILER_NAME, out_path.display());
        }
        return Ok(());
    }

    // Write C file
    fs::write(&c_file, &c_code).map_err(|e| {
        CompileError::driver(format!("cannot write '{}': {}", c_file.display(), e))
    })?;

    if opts.compile_only {
        // Compile to .o
        let obj_file = opts
            .output
            .clone()
            .unwrap_or_else(|| {
                opts.input
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(format!("{}.o", stem))
            });
        let mut cmd = Command::new(&opts.cc);
        cmd.arg("-c")
            .arg("-o")
            .arg(&obj_file)
            .arg(&c_file);

        add_mx_home_includes(&mut cmd);

        for flag in &opts.extra_cflags {
            cmd.arg(flag);
        }

        if opts.debug {
            cmd.args(["-g", "-O0", "-fno-omit-frame-pointer", "-fno-inline", "-gno-column-info"]);
        } else if opts.opt_level > 0 {
            cmd.arg(format!("-O{}", opts.opt_level));
        }
        cmd.args(["-ffunction-sections", "-fdata-sections"]);
        cmd.arg("-w"); // suppress warnings for generated code

        if opts.verbose {
            eprintln!("{}: {:?}", identity::COMPILER_NAME, cmd);
        }

        let output = cmd.output().map_err(|e| {
            CompileError::driver(format!("failed to run C compiler: {}", e))
        })?;

        if !output.status.success() {
            return Err(handle_cc_failure(&output.stderr, false, opts.diagnostics_json));
        }

        // Clean up (keep .c in debug mode for source mapping)
        if !opts.debug {
            let _ = fs::remove_file(&c_file);
        }

        if opts.verbose {
            eprintln!("{}: wrote {}", identity::COMPILER_NAME, obj_file.display());
        }
    } else {
        // Compile and link
        let exe_file = opts
            .output
            .clone()
            .unwrap_or_else(|| {
                opts.input
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(&*stem)
            });

        if opts.debug {
            // Debug: two-step compile+link so .o stays on disk for DWARF
            let obj_file = c_file.with_extension("o");

            // Step 1: compile .c → .o
            let mut compile_cmd = Command::new(&opts.cc);
            compile_cmd.arg("-c")
                .arg("-o").arg(&obj_file)
                .arg(&c_file)
                .args(["-g", "-O0", "-fno-omit-frame-pointer", "-fno-inline", "-gno-column-info"])
                .args(["-ffunction-sections", "-fdata-sections"])
                .arg("-w");
            add_mx_home_includes(&mut compile_cmd);
            for flag in &opts.extra_cflags {
                compile_cmd.arg(flag);
            }
            if opts.m2plus {
                if let Ok(c_src) = std::fs::read_to_string(&c_file) {
                    if c_src.contains("#define M2_USE_GC 1") && cfg!(target_os = "macos") {
                        compile_cmd.arg("-I/opt/homebrew/include");
                    }
                }
            }
            if opts.verbose {
                eprintln!("{}: {:?}", identity::COMPILER_NAME, compile_cmd);
            }
            let output = compile_cmd.output().map_err(|e| {
                CompileError::driver(format!("failed to run C compiler: {}", e))
            })?;
            if !output.status.success() {
                return Err(handle_cc_failure(&output.stderr, false, opts.diagnostics_json));
            }

            // Step 2: link .o → executable
            let mut link_cmd = Command::new(&opts.cc);
            link_cmd.arg("-o").arg(&exe_file)
                .arg(&obj_file)
                .arg("-g")
                .arg("-lm");
            if cfg!(target_os = "macos") {
                link_cmd.arg("-Wl,-dead_strip");
            } else {
                link_cmd.arg("-Wl,--gc-sections");
            }
            for extra in &opts.extra_c_files {
                link_cmd.arg(extra);
            }
            for path in &opts.link_paths {
                link_cmd.arg(format!("-L{}", path));
            }
            for lib in &opts.link_libs {
                link_cmd.arg(format!("-l{}", lib));
            }
            if cfg!(target_os = "macos") {
                for fw in &opts.frameworks {
                    link_cmd.arg("-framework");
                    link_cmd.arg(fw);
                }
            }
            if opts.m2plus {
                if let Ok(c_src) = std::fs::read_to_string(&c_file) {
                    if c_src.contains("#define M2_USE_THREADS 1") {
                        link_cmd.arg("-lpthread");
                    }
                    if c_src.contains("#define M2_USE_GC 1") {
                        if cfg!(target_os = "macos") {
                            link_cmd.arg("-L/opt/homebrew/lib");
                        }
                        link_cmd.arg("-lgc");
                    }
                }
            }
            if opts.verbose {
                eprintln!("{}: {:?}", identity::COMPILER_NAME, link_cmd);
            }
            let output = link_cmd.output().map_err(|e| {
                CompileError::driver(format!("failed to link: {}", e))
            })?;
            if !output.status.success() {
                return Err(handle_cc_failure(&output.stderr, true, opts.diagnostics_json));
            }

            // Step 3: dsymutil to create .dSYM bundle (macOS)
            if cfg!(target_os = "macos") {
                let mut dsym_cmd = Command::new("dsymutil");
                dsym_cmd.arg(&exe_file);
                if opts.verbose {
                    eprintln!("{}: {:?}", identity::COMPILER_NAME, dsym_cmd);
                }
                let _ = dsym_cmd.output(); // best-effort, don't fail if dsymutil missing
            }

            if opts.verbose {
                eprintln!("{}: wrote {}", identity::COMPILER_NAME, exe_file.display());
            }
        } else {
            // Release: single-step compile+link
            let mut cmd = Command::new(&opts.cc);
            cmd.arg("-o")
                .arg(&exe_file)
                .arg(&c_file)
                .arg("-lm");

            add_mx_home_includes(&mut cmd);

            for extra in &opts.extra_c_files {
                cmd.arg(extra);
            }
            for path in &opts.link_paths {
                cmd.arg(format!("-L{}", path));
            }
            for lib in &opts.link_libs {
                cmd.arg(format!("-l{}", lib));
            }
            for flag in &opts.extra_cflags {
                cmd.arg(flag);
            }
            if cfg!(target_os = "macos") {
                for fw in &opts.frameworks {
                    cmd.arg("-framework");
                    cmd.arg(fw);
                }
            }

            if opts.m2plus {
                if let Ok(c_src) = std::fs::read_to_string(&c_file) {
                    if c_src.contains("#define M2_USE_THREADS 1") {
                        cmd.arg("-lpthread");
                    }
                    if c_src.contains("#define M2_USE_GC 1") {
                        if cfg!(target_os = "macos") {
                            cmd.arg("-I/opt/homebrew/include");
                            cmd.arg("-L/opt/homebrew/lib");
                        }
                        cmd.arg("-lgc");
                    }
                }
            }

            if opts.opt_level > 0 {
                cmd.arg(format!("-O{}", opts.opt_level));
            }
            cmd.args(["-ffunction-sections", "-fdata-sections"]);
            if cfg!(target_os = "macos") {
                cmd.arg("-Wl,-dead_strip");
            } else {
                cmd.arg("-Wl,--gc-sections");
            }
            cmd.arg("-w");

            if opts.verbose {
                eprintln!("{}: {:?}", identity::COMPILER_NAME, cmd);
            }

            let output = cmd.output().map_err(|e| {
                CompileError::driver(format!("failed to run C compiler: {}", e))
            })?;

            if !output.status.success() {
                return Err(handle_cc_failure(&output.stderr, false, opts.diagnostics_json));
            }

            let _ = fs::remove_file(&c_file);

            if opts.verbose {
                eprintln!("{}: wrote {}", identity::COMPILER_NAME, exe_file.display());
            }
        }
    }

    Ok(())
}

/// Execute a JSON build plan file.
/// Each step in the plan becomes a compile() invocation.
pub fn compile_plan(plan_path: &str) -> CompileResult<()> {
    let content = fs::read_to_string(plan_path).map_err(|e| {
        CompileError::driver(format!("cannot read plan '{}': {}", plan_path, e))
    })?;

    let plan = crate::json::Json::parse(&content).map_err(|e| {
        CompileError::driver(format!("invalid JSON in '{}': {}", plan_path, e))
    })?;

    let version = plan.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
    if version != 1 {
        return Err(CompileError::driver(format!("unsupported plan version {}", version)));
    }

    let steps = plan.get("steps").and_then(|v| v.as_array()).ok_or_else(|| {
        CompileError::driver("plan missing 'steps' array".to_string())
    })?;

    // Resolve paths relative to the plan file's directory
    let plan_dir = std::path::Path::new(plan_path).parent().unwrap_or(std::path::Path::new("."));

    for (i, step) in steps.iter().enumerate() {
        let entry = step.as_str_or("entry", "");
        if entry.is_empty() {
            return Err(CompileError::driver(format!("step {} missing 'entry'", i)));
        }

        let mut opts = CompileOptions::default();
        opts.input = plan_dir.join(&entry);
        opts.m2plus = step.as_bool_or("m2plus", false);

        if let Some(out) = step.get("output").and_then(|v| v.as_str()) {
            opts.output = Some(plan_dir.join(out));
        }

        opts.emit_c = step.as_bool_or("emit_c", false);
        opts.compile_only = step.as_bool_or("compile_only", false);

        if let Some(cc) = step.get("cc").and_then(|v| v.as_str()) {
            opts.cc = cc.to_string();
        }

        if let Some(opt) = step.get("opt_level").and_then(|v| v.as_i64()) {
            opts.opt_level = opt as u8;
        }

        for inc in step.as_string_array("includes") {
            opts.include_paths.push(plan_dir.join(&inc));
        }

        for extra in step.as_string_array("extra_c") {
            opts.extra_c_files.push(plan_dir.join(&extra));
        }

        for lib in step.as_string_array("link_libs") {
            opts.link_libs.push(lib);
        }

        for lp in step.as_string_array("link_paths") {
            opts.link_paths.push(lp);
        }

        eprintln!("{}: plan step {}: compile {}", identity::COMPILER_NAME, i, opts.input.display());
        compile(&opts)?;
    }

    Ok(())
}
