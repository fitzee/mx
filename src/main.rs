#![allow(dead_code, unused_imports, unused_variables, unused_parens)]

mod analyze;
mod ast;
mod build;
mod builtins;
mod cfg;
mod codegen_c;
mod codegen_llvm;
mod driver;
mod hir;
mod hir_build;
mod errors;
mod json;
mod lang_docs;
mod lexer;
mod lsp;
mod parser;
mod project_resolver;
mod sema;
mod stdlib;
mod symtab;
mod target;
mod token;
mod types;
mod identity;

use std::path::PathBuf;
use std::process;

use driver::CompileOptions;
use identity::{COMPILER_NAME, COMPILER_ID, VERSION, BUILD_DIR};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle zero-arg and help cases
    if args.len() < 2 || args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{} - Modula-2 compiler (PIM4)", COMPILER_NAME);
        eprintln!("Usage: {} [options] file.mod", COMPILER_NAME);
        eprintln!("       {} build [--release] [-g] [-v] [--cc <cmd>] [--feature <name>]...", COMPILER_NAME);
        eprintln!("       {} run [--release] [-v] [-- <args>...]", COMPILER_NAME);
        eprintln!("       {} test [-v] [--feature <name>]...", COMPILER_NAME);
        eprintln!("       {} clean", COMPILER_NAME);
        eprintln!("       {} init [<name>]", COMPILER_NAME);
        eprintln!();
        eprintln!("Subcommands:");
        eprintln!("  build          Build the project (reads m2.toml manifest)");
        eprintln!("  run            Build and run the project");
        eprintln!("  test           Build and run tests");
        eprintln!("  clean          Remove build artifacts (.{}/)", BUILD_DIR);
        eprintln!("  init [<name>]  Create a new project (default: current directory name)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -o <file>      Output file name");
        eprintln!("  -c             Compile only (produce .o, no linking)");
        eprintln!("  --emit-c       Output C code only (do not invoke C compiler)");
        eprintln!("  --emit-llvm    Output LLVM IR (.ll) instead of C");
        eprintln!("  --cfg          Output control flow graph as DOT (.dot)");
        eprintln!("  --llvm         Use LLVM backend (compile via clang)");
        eprintln!("  -I <path>      Add module search path");
        eprintln!("  -O<n>          Optimization level (0-3)");
        eprintln!("  -v             Verbose output");
        eprintln!("  --cc <cmd>     C compiler to use (default: cc)");
        eprintln!("  --m2plus       Enable Modula-2+ extensions");
        eprintln!("  --case-insensitive  Case-insensitive identifiers (default: case-sensitive)");
        eprintln!("  --diagnostics-json  Emit errors as JSONL to stderr");
        eprintln!("  --feature <name>    Enable a feature for conditional compilation");
        eprintln!("  --lsp               Start LSP server (JSON-RPC over stdio)");
        eprintln!("  -g, --debug    Compile with debug info (DWARF via #line mapping)");
        eprintln!("  --sanitize     Enable AddressSanitizer + UndefinedBehaviorSanitizer");
        eprintln!("  -l <lib>       Link with library");
        eprintln!("  -L <path>      Add library search path");
        eprintln!("  --target <triple>  Set target (e.g. x86_64-linux, aarch64-darwin)");
        eprintln!("  --cflag <arg>  Pass flag to C compiler");
        eprintln!("  file.c/.o/.a   Extra C/object/archive files to link");
        eprintln!();
        eprintln!("  --version-json  Print version info as JSON");
        eprintln!("  --print-targets Print supported target triples");
        eprintln!("  compile --plan <file.json>  Execute a build plan");
        eprintln!();
        eprintln!("  -h, --help     Show this help message");
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    // --version-json: machine-readable version output
    if args.iter().any(|a| a == "--version-json") {
        let ti = target::TargetInfo::from_host();
        let version_json = json::Json::obj(vec![
            ("name", json::Json::str_val(COMPILER_ID)),
            ("version", json::Json::str_val(VERSION)),
            ("target", json::Json::str_val(&ti.triple)),
            ("plan_version", json::Json::int_val(1)),
            ("target_info", json::Json::obj(vec![
                ("triple", json::Json::str_val(&ti.triple)),
                ("arch", json::Json::str_val(&ti.arch.to_string())),
                ("os", json::Json::str_val(&ti.os.to_string())),
                ("pointer_bits", json::Json::int_val(ti.pointer_bits as i64)),
                ("endian", json::Json::str_val(match ti.endian {
                    target::Endianness::Little => "little",
                    target::Endianness::Big => "big",
                })),
                ("c_abi", json::Json::str_val(match ti.c_abi {
                    target::CAbi::SysV => "sysv",
                    target::CAbi::Darwin => "darwin",
                })),
                ("supports_setjmp", json::Json::bool_val(ti.supports_setjmp)),
                ("int_layout", json::Json::obj(vec![
                    ("integer_bytes", json::Json::int_val(ti.int_layout.integer_bytes as i64)),
                    ("cardinal_bytes", json::Json::int_val(ti.int_layout.cardinal_bytes as i64)),
                    ("longint_bytes", json::Json::int_val(ti.int_layout.longint_bytes as i64)),
                    ("longcard_bytes", json::Json::int_val(ti.int_layout.longcard_bytes as i64)),
                    ("real_bytes", json::Json::int_val(ti.int_layout.real_bytes as i64)),
                    ("longreal_bytes", json::Json::int_val(ti.int_layout.longreal_bytes as i64)),
                    ("bitset_bytes", json::Json::int_val(ti.int_layout.bitset_bytes as i64)),
                ])),
                ("alignments", json::Json::obj(vec![
                    ("pointer", json::Json::int_val(ti.alignments.pointer_align as i64)),
                    ("char", json::Json::int_val(ti.alignments.char_align as i64)),
                    ("int", json::Json::int_val(ti.alignments.int_align as i64)),
                    ("long", json::Json::int_val(ti.alignments.long_align as i64)),
                    ("float", json::Json::int_val(ti.alignments.float_align as i64)),
                    ("double", json::Json::int_val(ti.alignments.double_align as i64)),
                ])),
                ("supported_targets", json::Json::arr(
                    target::supported_targets().iter()
                        .map(|s| json::Json::str_val(s))
                        .collect()
                )),
            ])),
            ("capabilities", json::Json::obj(vec![
                ("emit_c", json::Json::bool_val(true)),
                ("emit_llvm", json::Json::bool_val(true)),
                ("llvm", json::Json::bool_val(true)),
                ("compile_plan", json::Json::bool_val(true)),
                ("m2plus", json::Json::bool_val(true)),
                ("ffi_c", json::Json::bool_val(true)),
                ("exportc", json::Json::bool_val(true)),
                ("diagnostics_json", json::Json::bool_val(true)),
                ("features", json::Json::bool_val(true)),
                ("cfg", json::Json::bool_val(true)),
                ("target", json::Json::bool_val(true)),
            ])),
            ("stdlib", json::Json::arr(
                stdlib::stdlib_module_names().iter()
                    .map(|s| json::Json::str_val(s))
                    .collect()
            )),
        ]);
        println!("{}", version_json.serialize());
        process::exit(0);
    }

    // --print-targets: list supported targets
    if args.iter().any(|a| a == "--print-targets") {
        let host = target::TargetInfo::from_host();
        for triple in target::supported_targets() {
            if triple == host.triple || (host.triple.contains("apple") && triple.contains("apple") && triple.contains(&host.arch.to_string())) {
                println!("{} (host, default)", triple);
            } else {
                println!("{}", triple);
            }
        }
        println!("# {} emits portable C; cross-compile by setting --cc to a cross compiler.", COMPILER_NAME);
        process::exit(0);
    }

    // --lsp: start LSP server
    if args.iter().any(|a| a == "--lsp") {
        let m2plus = args.iter().any(|a| a == "--m2plus");
        let mut include_paths = Vec::new();
        let mut i = 1;
        while i < args.len() {
            if args[i] == "-I" && i + 1 < args.len() {
                include_paths.push(std::path::PathBuf::from(&args[i + 1]));
                i += 2;
            } else {
                i += 1;
            }
        }
        let code = lsp::run_lsp_server(m2plus, include_paths);
        process::exit(code);
    }

    // compile --plan <file>: build-plan mode
    if args.len() >= 3 && args[1] == "compile" && args[2] == "--plan" {
        if args.len() < 4 {
            eprintln!("{}: compile --plan requires a JSON file argument", COMPILER_NAME);
            process::exit(1);
        }
        match driver::compile_plan(&args[3]) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
        return;
    }

    // Subcommand routing: build / run / test / clean
    if args.len() >= 2 {
        match args[1].as_str() {
            "build" | "run" | "test" | "clean" | "init" => {
                run_subcommand(&args);
                return;
            }
            _ => {}
        }
    }

    let mut opts = CompileOptions::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: -o requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.output = Some(PathBuf::from(&args[i]));
            }
            "-c" => opts.compile_only = true,
            "--emit-c" => opts.emit_c = true,
            "--emit-llvm" => opts.emit_llvm = true,
            "--cfg" => opts.emit_cfg = true,
            "--llvm" => { opts.use_llvm = true; opts.emit_llvm = true; },
            "-I" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: -I requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.include_paths.push(PathBuf::from(&args[i]));
            }
            "-v" => opts.verbose = true,
            "-g" | "--debug" => opts.debug = true,
            "--m2plus" => opts.m2plus = true,
            "--sanitize" => opts.sanitize = true,
            "--case-insensitive" => opts.case_sensitive = false,
            "--diagnostics-json" => opts.diagnostics_json = true,
            "--feature" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --feature requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.features.push(args[i].clone());
            }
            "--cc" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --cc requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.cc = args[i].clone();
            }
            arg if arg.starts_with("-O") => {
                opts.opt_level = arg[2..].parse().unwrap_or(2);
            }
            "-l" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: -l requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.link_libs.push(args[i].clone());
            }
            arg if arg.starts_with("-l") => {
                opts.link_libs.push(arg[2..].to_string());
            }
            "-L" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: -L requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.link_paths.push(args[i].clone());
            }
            arg if arg.starts_with("-L") => {
                opts.link_paths.push(arg[2..].to_string());
            }
            "--cflag" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --cflag requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.extra_cflags.push(args[i].clone());
            }
            "--target" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --target requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.target_triple = Some(args[i].clone());
            }
            "--emit-per-module" => {
                opts.emit_per_module = true;
                opts.emit_c = true;  // per-module implies emit-c (no linking by driver)
            }
            "--out-dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --out-dir requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                opts.out_dir = Some(PathBuf::from(&args[i]));
            }
            arg if arg.starts_with('-') => {
                eprintln!("{}: unknown option '{}'", COMPILER_NAME, arg);
                process::exit(1);
            }
            arg if arg.ends_with(".c") || arg.ends_with(".o") || arg.ends_with(".a") => {
                opts.extra_c_files.push(PathBuf::from(arg));
            }
            _ => {
                opts.input = PathBuf::from(&args[i]);
            }
        }
        i += 1;
    }

    if opts.input.as_os_str().is_empty() {
        eprintln!("{}: no input file", COMPILER_NAME);
        process::exit(1);
    }

    match driver::compile(&opts) {
        Ok(()) => {}
        Err(e) => {
            if !opts.diagnostics_json {
                eprintln!("{}", e);
            }
            process::exit(1);
        }
    }
}

fn current_target() -> String {
    target::TargetInfo::from_host().triple
}

fn run_init(args: &[String]) {
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("{}: cannot determine working directory: {}", COMPILER_NAME, e);
        process::exit(1);
    });

    // Optional project name from args[2], else use current directory name
    let name = if args.len() > 2 && !args[2].starts_with('-') {
        args[2].clone()
    } else {
        cwd.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("myproject")
            .to_string()
    };

    // Capitalize first letter for module name
    let mod_name = {
        let mut chars = name.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            None => name.clone(),
        }
    };

    let manifest_path = cwd.join("m2.toml");
    if manifest_path.exists() {
        eprintln!("{}: m2.toml already exists in this directory", COMPILER_NAME);
        process::exit(1);
    }

    // Create m2.toml
    let manifest = format!("\
# m2.toml - Modula-2 project manifest
name={name}
version=0.1.0
entry=src/Main.mod
m2plus=true
includes=src

[deps]

[cc]
# cflags=
# ldflags=
# libs=
# extra-c=
# frameworks=

[test]
entry=tests/Main.mod
includes=tests
");
    std::fs::write(&manifest_path, &manifest).unwrap_or_else(|e| {
        eprintln!("{}: failed to write m2.toml: {}", COMPILER_NAME, e);
        process::exit(1);
    });

    // Create src/Main.mod
    let src_dir = cwd.join("src");
    std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
        eprintln!("{}: failed to create src/: {}", COMPILER_NAME, e);
        process::exit(1);
    });
    let src_main = format!("\
MODULE {mod_name};

FROM InOut IMPORT WriteString, WriteLn;

BEGIN
  WriteString(\"Hello from {mod_name}!\");
  WriteLn;
END {mod_name}.
");
    std::fs::write(src_dir.join("Main.mod"), &src_main).unwrap_or_else(|e| {
        eprintln!("{}: failed to write src/Main.mod: {}", COMPILER_NAME, e);
        process::exit(1);
    });

    // Create tests/Main.mod
    let tests_dir = cwd.join("tests");
    std::fs::create_dir_all(&tests_dir).unwrap_or_else(|e| {
        eprintln!("{}: failed to create tests/: {}", COMPILER_NAME, e);
        process::exit(1);
    });
    let test_main = format!("\
MODULE {mod_name}Test;

FROM InOut IMPORT WriteString, WriteLn;

BEGIN
  WriteString(\"Tests passed.\");
  WriteLn;
END {mod_name}Test.
");
    std::fs::write(tests_dir.join("Main.mod"), &test_main).unwrap_or_else(|e| {
        eprintln!("{}: failed to write tests/Main.mod: {}", COMPILER_NAME, e);
        process::exit(1);
    });

    eprintln!("{}: initialized project '{}'", COMPILER_NAME, name);
    eprintln!("  created m2.toml");
    eprintln!("  created src/Main.mod");
    eprintln!("  created tests/Main.mod");
}

fn run_subcommand(args: &[String]) {
    let subcmd = &args[1];
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("{}: cannot determine working directory: {}", COMPILER_NAME, e);
        process::exit(1);
    });

    // init is special — creates a new project, no existing project needed
    if subcmd == "init" {
        run_init(&args);
        return;
    }

    // clean is special — no need to load project context
    if subcmd == "clean" {
        let root = match project_resolver::find_project_root(&cwd) {
            Some(r) => r,
            None => {
                eprintln!("{}: no m2.toml found in current or parent directories", COMPILER_NAME);
                process::exit(1);
            }
        };
        match build::clean_project(&root) {
            Ok(()) => eprintln!("{}: cleaned", COMPILER_NAME),
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
        return;
    }

    // Parse subcommand flags
    let mut verbose = false;
    let mut release = false;
    let mut debug = false;
    let mut use_llvm = false;
    let mut sanitize = false;
    let mut cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let mut features: Vec<String> = Vec::new();
    let mut run_args: Vec<String> = Vec::new();
    let mut saw_dashdash = false;
    let mut target_triple: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        if saw_dashdash {
            run_args.push(args[i].clone());
            i += 1;
            continue;
        }
        match args[i].as_str() {
            "--" => saw_dashdash = true,
            "-v" => verbose = true,
            "--release" => release = true,
            "-g" | "--debug" => debug = true,
            "--llvm" => use_llvm = true,
            "--sanitize" => sanitize = true,
            "--cc" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --cc requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                cc = args[i].clone();
            }
            "--feature" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --feature requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                features.push(args[i].clone());
            }
            "--target" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{}: --target requires an argument", COMPILER_NAME);
                    process::exit(1);
                }
                target_triple = Some(args[i].clone());
            }
            arg if arg.starts_with('-') => {
                eprintln!("{} {}: unknown option '{}'", COMPILER_NAME, subcmd, arg);
                process::exit(1);
            }
            _ => {
                eprintln!("{} {}: unexpected argument '{}'", COMPILER_NAME, subcmd, args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Load project context
    let root = match project_resolver::find_project_root(&cwd) {
        Some(r) => r,
        None => {
            eprintln!("{}: no m2.toml found in current or parent directories", COMPILER_NAME);
            process::exit(1);
        }
    };

    let ctx = match project_resolver::ProjectContext::load(&root, &[]) {
        Some(c) => c,
        None => {
            eprintln!("{}: failed to load project from {}", COMPILER_NAME, root.display());
            process::exit(1);
        }
    };

    let is_test = subcmd == "test";
    let is_run = subcmd == "run" || is_test;

    // For test subcommand, use the test entry and include test paths
    let mut include_paths = ctx.include_paths.clone();
    let manifest = if is_test {
        // Add test includes
        for inc in &ctx.manifest.test.includes {
            let p = root.join(inc);
            if p.is_dir() && !include_paths.contains(&p) {
                include_paths.push(p);
            }
        }
        // Create a modified manifest with test entry
        let m = project_resolver::Manifest {
            name: format!("{}_test", ctx.manifest.name),
            version: ctx.manifest.version.clone(),
            entry: ctx.manifest.test.entry.clone(),
            m2plus: ctx.manifest.m2plus,
            includes: ctx.manifest.includes.clone(),
            deps: ctx.manifest.deps.iter().map(|d| project_resolver::DepEntry {
                name: d.name.clone(),
                source: match &d.source {
                    project_resolver::DepSource::Local(p) => project_resolver::DepSource::Local(p.clone()),
                    project_resolver::DepSource::Registry(v) => project_resolver::DepSource::Registry(v.clone()),
                    project_resolver::DepSource::Installed => project_resolver::DepSource::Installed,
                },
            }).collect(),
            cc: project_resolver::CcSection {
                cflags: ctx.manifest.cc.cflags.clone(),
                ldflags: ctx.manifest.cc.ldflags.clone(),
                libs: ctx.manifest.cc.libs.clone(),
                extra_c: ctx.manifest.cc.extra_c.clone(),
                frameworks: ctx.manifest.cc.frameworks.clone(),
            },
            feature_cc: ctx.manifest.feature_cc.clone(),
            test: project_resolver::TestSection::default(),
            backend: None,
        };
        m
    } else {
        // Transfer ownership-like copy for build/run
        project_resolver::Manifest {
            name: ctx.manifest.name.clone(),
            version: ctx.manifest.version.clone(),
            entry: ctx.manifest.entry.clone(),
            m2plus: ctx.manifest.m2plus,
            includes: ctx.manifest.includes.clone(),
            deps: ctx.manifest.deps.iter().map(|d| project_resolver::DepEntry {
                name: d.name.clone(),
                source: match &d.source {
                    project_resolver::DepSource::Local(p) => project_resolver::DepSource::Local(p.clone()),
                    project_resolver::DepSource::Registry(v) => project_resolver::DepSource::Registry(v.clone()),
                    project_resolver::DepSource::Installed => project_resolver::DepSource::Installed,
                },
            }).collect(),
            cc: project_resolver::CcSection {
                cflags: ctx.manifest.cc.cflags.clone(),
                ldflags: ctx.manifest.cc.ldflags.clone(),
                libs: ctx.manifest.cc.libs.clone(),
                extra_c: ctx.manifest.cc.extra_c.clone(),
                frameworks: ctx.manifest.cc.frameworks.clone(),
            },
            feature_cc: ctx.manifest.feature_cc.clone(),
            test: project_resolver::TestSection::default(),
            backend: None,
        }
    };

    // Check manifest for backend=llvm
    let manifest_llvm = ctx.manifest.backend.as_deref() == Some("llvm");

    let config = build::BuildConfig {
        root: root.clone(),
        manifest,
        include_paths,
        cc,
        opt_level: if release { 2 } else { 0 },
        verbose,
        features,
        debug,
        use_llvm: use_llvm || manifest_llvm,
        sanitize,
        target_triple,
    };

    match build::build_project(&config, is_run, &run_args) {
        Ok(result) => {
            if !is_run && !result.up_to_date {
                eprintln!("{}: built {}", COMPILER_NAME, result.artifact.display());
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
