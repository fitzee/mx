use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::CompilationUnit;
use crate::codegen::CodeGen;
use crate::errors::{CompileError, CompileResult};
use crate::lexer::Lexer;
use crate::parser::Parser;

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
    results
}

/// Build a driver error from C compiler failure, suppressing raw C errors
/// unless M2C_SHOW_C_ERRORS=1 is set.
/// Split multi-TU C output on markers and write per-module files + manifest.
///
/// Marker structure in the amalgamated C:
///   /* M2C_HEADER_BEGIN */  ... runtime header ...  /* M2C_HEADER_END */
///   /* M2C_MODULE_BEGIN ModName */  ... types/protos ...
///   /* M2C_MODULE_DEFS ModName */   ... vars/bodies/init ...
///   /* M2C_MODULE_END ModName */
///   ...more modules...
///   /* M2C_MAIN_BEGIN MainName */  ... main module ...  /* M2C_MAIN_END */
///
/// Output files:
///   <out_dir>/_common.h   — header + all module declaration sections
///   <out_dir>/<Module>.c  — #include "_common.h" + module body section
///   <out_dir>/_main.c     — #include "_common.h" + main module section
///   <out_dir>/_manifest.txt — list of .c files
fn write_per_module_files(c_code: &str, opts: &CompileOptions) -> CompileResult<()> {
    let out_dir = opts.out_dir.clone().unwrap_or_else(|| {
        opts.input.parent().unwrap_or(Path::new(".")).join("m2c_out")
    });
    fs::create_dir_all(&out_dir).map_err(|e| {
        CompileError::driver(format!("cannot create output dir '{}': {}", out_dir.display(), e))
    })?;

    // Extract the header section (runtime header)
    let header_begin = c_code.find("/* M2C_HEADER_BEGIN */\n")
        .unwrap_or(0);
    let header_end_marker = "/* M2C_HEADER_END */\n";
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
    while let Some(mb_offset) = remaining[search_pos..].find("/* M2C_MODULE_BEGIN ") {
        let mb_start = search_pos + mb_offset;
        // Extract module name from marker
        let name_start = mb_start + "/* M2C_MODULE_BEGIN ".len();
        let name_end = remaining[name_start..].find(" */")
            .map(|p| name_start + p)
            .unwrap_or(name_start);
        let mod_name = remaining[name_start..name_end].to_string();

        // Find MODULE_DEFS marker
        let defs_marker = format!("/* M2C_MODULE_DEFS {} */\n", mod_name);
        let end_marker = format!("/* M2C_MODULE_END {} */\n", mod_name);

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
    let main_begin_marker = "/* M2C_MAIN_BEGIN ";
    let main_end_marker = "/* M2C_MAIN_END */\n";
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
        eprintln!("m2c: wrote {} per-module files to {}", manifest_lines.len(), out_dir.display());
    }

    Ok(())
}

fn cc_failure_error(stderr: &[u8]) -> CompileError {
    let show_c = std::env::var("M2C_SHOW_C_ERRORS").map_or(false, |v| v == "1");
    if show_c {
        let msg = String::from_utf8_lossy(stderr);
        CompileError::driver(format!("C backend failed:\n{}", msg.trim()))
    } else {
        CompileError::driver(
            "C backend failed (internal error). Re-run with M2C_SHOW_C_ERRORS=1 for details."
        )
    }
}

/// Parse a source file and return the compilation unit
fn parse_file(path: &Path, case_sensitive: bool) -> CompileResult<CompilationUnit> {
    let source = fs::read_to_string(path).map_err(|e| {
        CompileError::driver(format!("cannot read '{}': {}", path.display(), e))
    })?;
    let filename = path.to_string_lossy().to_string();
    let mut lexer = Lexer::new(&source, &filename);
    lexer.set_case_sensitive(case_sensitive);
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
        eprintln!("m2c: compiling {}", filename);
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
        eprintln!("m2c: {} tokens", tokens.len());
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
        eprintln!("m2c: parsed successfully");
    }

    // If this is an implementation module, look for the corresponding definition module
    if let CompilationUnit::ImplementationModule(ref m) = unit {
        if let Some(def_path) = find_def_file(&m.name, &opts.input, &opts.include_paths) {
            if opts.verbose {
                eprintln!("m2c: found definition module: {}", def_path.display());
            }
            let _def_unit = parse_file(&def_path, opts.case_sensitive)?;
        }
    }

    // Generate C
    let mut codegen = CodeGen::new();
    codegen.set_m2plus(opts.m2plus);
    codegen.set_debug(opts.debug);
    codegen.multi_tu = opts.emit_per_module;

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
                all_imported_modules.push(mod_name.clone());
            }
        }
    }

    // First pass: parse and register definition modules for all non-stdlib imports
    // This allows sema to resolve types and procedures from imported modules
    let mut registered_defs = std::collections::HashSet::new();
    let mut def_queue: Vec<String> = all_imported_modules.clone();
    while let Some(mod_name) = def_queue.pop() {
        if crate::stdlib::is_stdlib_module(&mod_name) || registered_defs.contains(&mod_name) {
            continue;
        }
        if let Some(def_path) = find_def_file(&mod_name, &opts.input, &opts.include_paths) {
            if opts.verbose {
                eprintln!("m2c: found definition module for {}: {}", mod_name, def_path.display());
            }
            let def_unit = parse_file(&def_path, opts.case_sensitive)?;
            if let CompilationUnit::DefinitionModule(def_mod) = def_unit {
                // Transitively discover imports of this def module's corresponding impl
                for imp in &def_mod.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        if !registered_defs.contains(from_mod) {
                            def_queue.push(from_mod.clone());
                        }
                    } else {
                        for name in &imp.names {
                            if !registered_defs.contains(name) {
                                def_queue.push(name.clone());
                            }
                        }
                    }
                }
                codegen.register_def_module(&def_mod);
                registered_defs.insert(mod_name.clone());
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
                eprintln!("m2c: skipping .mod lookup for foreign module {}", mod_name);
            }
            loaded_modules.insert(mod_name.clone());
            continue;
        }
        let mod_candidates = find_mod_file_candidates(&mod_name, &opts.input, &opts.include_paths);
        let mut found_impl = None;
        for mod_path in &mod_candidates {
            if opts.verbose {
                eprintln!("m2c: trying implementation module for {}: {}", mod_name, mod_path.display());
            }
            let mod_unit = parse_file(mod_path, opts.case_sensitive)?;
            if let CompilationUnit::ImplementationModule(imp) = mod_unit {
                found_impl = Some(imp);
                if opts.verbose {
                    eprintln!("m2c: found implementation module for {}: {}", mod_name, mod_path.display());
                }
                break;
            }
        }
        if let Some(imp_mod) = found_impl {
            // Transitively discover dependencies of this implementation module
            for imp in &imp_mod.imports {
                if let Some(ref from_mod) = imp.from_module {
                    if !loaded_modules.contains(from_mod) && !registered_defs.contains(from_mod) {
                        // Register the def module first
                        if !crate::stdlib::is_stdlib_module(from_mod) {
                            if let Some(dep_def_path) = find_def_file(from_mod, &opts.input, &opts.include_paths) {
                                if opts.verbose {
                                    eprintln!("m2c: found definition module for {}: {}", from_mod, dep_def_path.display());
                                }
                                let dep_def_unit = parse_file(&dep_def_path, opts.case_sensitive)?;
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
                        if !loaded_modules.contains(name) {
                            // Also register the def module if not already done
                            if !registered_defs.contains(name) && !crate::stdlib::is_stdlib_module(name) {
                                if let Some(dep_def_path) = find_def_file(name, &opts.input, &opts.include_paths) {
                                    if opts.verbose {
                                        eprintln!("m2c: found definition module for {}: {}", name, dep_def_path.display());
                                    }
                                    if let Ok(dep_def_unit) = parse_file(&dep_def_path, opts.case_sensitive) {
                                        if let CompilationUnit::DefinitionModule(dep_def) = dep_def_unit {
                                            codegen.register_def_module(&dep_def);
                                            registered_defs.insert(name.clone());
                                        }
                                    }
                                }
                            }
                            mod_queue.push(name.clone());
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
        eprintln!("m2c: C code generated ({} bytes)", c_code.len());
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
            eprintln!("m2c: wrote {}", out_path.display());
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

        for flag in &opts.extra_cflags {
            cmd.arg(flag);
        }

        if opts.debug {
            cmd.args(["-g", "-O0", "-fno-omit-frame-pointer", "-fno-inline", "-gno-column-info"]);
        } else if opts.opt_level > 0 {
            cmd.arg(format!("-O{}", opts.opt_level));
        }
        cmd.arg("-w"); // suppress warnings for generated code

        if opts.verbose {
            eprintln!("m2c: {:?}", cmd);
        }

        let output = cmd.output().map_err(|e| {
            CompileError::driver(format!("failed to run C compiler: {}", e))
        })?;

        if !output.status.success() {
            return Err(cc_failure_error(&output.stderr));
        }

        // Clean up (keep .c in debug mode for source mapping)
        if !opts.debug {
            let _ = fs::remove_file(&c_file);
        }

        if opts.verbose {
            eprintln!("m2c: wrote {}", obj_file.display());
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
                .arg("-w");
            for flag in &opts.extra_cflags {
                compile_cmd.arg(flag);
            }
            if opts.m2plus {
                if let Ok(c_src) = std::fs::read_to_string(&c_file) {
                    if c_src.contains("#define M2_USE_GC 1") {
                        compile_cmd.arg("-I/opt/homebrew/include");
                    }
                }
            }
            if opts.verbose {
                eprintln!("m2c: {:?}", compile_cmd);
            }
            let output = compile_cmd.output().map_err(|e| {
                CompileError::driver(format!("failed to run C compiler: {}", e))
            })?;
            if !output.status.success() {
                return Err(cc_failure_error(&output.stderr));
            }

            // Step 2: link .o → executable
            let mut link_cmd = Command::new(&opts.cc);
            link_cmd.arg("-o").arg(&exe_file)
                .arg(&obj_file)
                .arg("-g")
                .arg("-lm");
            for extra in &opts.extra_c_files {
                link_cmd.arg(extra);
            }
            for path in &opts.link_paths {
                link_cmd.arg(format!("-L{}", path));
            }
            for lib in &opts.link_libs {
                link_cmd.arg(format!("-l{}", lib));
            }
            for fw in &opts.frameworks {
                link_cmd.arg("-framework");
                link_cmd.arg(fw);
            }
            if opts.m2plus {
                if let Ok(c_src) = std::fs::read_to_string(&c_file) {
                    if c_src.contains("#define M2_USE_THREADS 1") {
                        link_cmd.arg("-lpthread");
                    }
                    if c_src.contains("#define M2_USE_GC 1") {
                        link_cmd.arg("-L/opt/homebrew/lib");
                        link_cmd.arg("-lgc");
                    }
                }
            }
            if opts.verbose {
                eprintln!("m2c: {:?}", link_cmd);
            }
            let output = link_cmd.output().map_err(|e| {
                CompileError::driver(format!("failed to link: {}", e))
            })?;
            if !output.status.success() {
                return Err(cc_failure_error(&output.stderr));
            }

            // Step 3: dsymutil to create .dSYM bundle (macOS)
            if cfg!(target_os = "macos") {
                let mut dsym_cmd = Command::new("dsymutil");
                dsym_cmd.arg(&exe_file);
                if opts.verbose {
                    eprintln!("m2c: {:?}", dsym_cmd);
                }
                let _ = dsym_cmd.output(); // best-effort, don't fail if dsymutil missing
            }

            if opts.verbose {
                eprintln!("m2c: wrote {}", exe_file.display());
            }
        } else {
            // Release: single-step compile+link
            let mut cmd = Command::new(&opts.cc);
            cmd.arg("-o")
                .arg(&exe_file)
                .arg(&c_file)
                .arg("-lm");

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
            for fw in &opts.frameworks {
                cmd.arg("-framework");
                cmd.arg(fw);
            }

            if opts.m2plus {
                if let Ok(c_src) = std::fs::read_to_string(&c_file) {
                    if c_src.contains("#define M2_USE_THREADS 1") {
                        cmd.arg("-lpthread");
                    }
                    if c_src.contains("#define M2_USE_GC 1") {
                        cmd.arg("-I/opt/homebrew/include");
                        cmd.arg("-L/opt/homebrew/lib");
                        cmd.arg("-lgc");
                    }
                }
            }

            if opts.opt_level > 0 {
                cmd.arg(format!("-O{}", opts.opt_level));
            }
            cmd.arg("-w");

            if opts.verbose {
                eprintln!("m2c: {:?}", cmd);
            }

            let output = cmd.output().map_err(|e| {
                CompileError::driver(format!("failed to run C compiler: {}", e))
            })?;

            if !output.status.success() {
                return Err(cc_failure_error(&output.stderr));
            }

            let _ = fs::remove_file(&c_file);

            if opts.verbose {
                eprintln!("m2c: wrote {}", exe_file.display());
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

        eprintln!("m2c: plan step {}: compile {}", i, opts.input.display());
        compile(&opts)?;
    }

    Ok(())
}
