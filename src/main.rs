#![allow(dead_code, unused_imports, unused_variables, unused_parens)]

mod analyze;
mod ast;
mod build;
mod builtins;
mod codegen;
mod driver;
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
mod token;
mod types;

use std::path::PathBuf;
use std::process;

use driver::CompileOptions;

const M2C_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle zero-arg and help cases
    if args.len() < 2 || args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("m2c - Modula-2 to C Compiler (PIM4)");
        eprintln!("Usage: m2c [options] file.mod");
        eprintln!("       m2c build [--release] [-g] [-v] [--cc <cmd>] [--feature <name>]...");
        eprintln!("       m2c run [--release] [-v] [-- <args>...]");
        eprintln!("       m2c test [-v] [--feature <name>]...");
        eprintln!("       m2c clean");
        eprintln!("       m2c init [<name>]");
        eprintln!();
        eprintln!("Subcommands:");
        eprintln!("  build          Build the project (reads m2.toml manifest)");
        eprintln!("  run            Build and run the project");
        eprintln!("  test           Build and run tests");
        eprintln!("  clean          Remove build artifacts (.m2c/)");
        eprintln!("  init [<name>]  Create a new project (default: current directory name)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -o <file>      Output file name");
        eprintln!("  -c             Compile only (produce .o, no linking)");
        eprintln!("  --emit-c       Output C code only (do not invoke C compiler)");
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
        eprintln!("  -l <lib>       Link with library");
        eprintln!("  -L <path>      Add library search path");
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
        let target = current_target();
        let version_json = json::Json::obj(vec![
            ("name", json::Json::str_val("m2c")),
            ("version", json::Json::str_val(M2C_VERSION)),
            ("target", json::Json::str_val(&target)),
            ("plan_version", json::Json::int_val(1)),
            ("capabilities", json::Json::obj(vec![
                ("emit_c", json::Json::bool_val(true)),
                ("compile_plan", json::Json::bool_val(true)),
                ("m2plus", json::Json::bool_val(true)),
                ("ffi_c", json::Json::bool_val(true)),
                ("exportc", json::Json::bool_val(true)),
                ("diagnostics_json", json::Json::bool_val(true)),
                ("features", json::Json::bool_val(true)),
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
        // m2c transpiles to C, so it supports any target the system cc supports.
        // We report the host as the default and note the cross-compile capability.
        let target = current_target();
        println!("{} (host, default)", target);
        println!("# m2c emits portable C; cross-compile by setting --cc to a cross compiler.");
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
            eprintln!("m2c: compile --plan requires a JSON file argument");
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
                    eprintln!("m2c: -o requires an argument");
                    process::exit(1);
                }
                opts.output = Some(PathBuf::from(&args[i]));
            }
            "-c" => opts.compile_only = true,
            "--emit-c" => opts.emit_c = true,
            "-I" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m2c: -I requires an argument");
                    process::exit(1);
                }
                opts.include_paths.push(PathBuf::from(&args[i]));
            }
            "-v" => opts.verbose = true,
            "-g" | "--debug" => opts.debug = true,
            "--m2plus" => opts.m2plus = true,
            "--case-insensitive" => opts.case_sensitive = false,
            "--diagnostics-json" => opts.diagnostics_json = true,
            "--feature" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m2c: --feature requires an argument");
                    process::exit(1);
                }
                opts.features.push(args[i].clone());
            }
            "--cc" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m2c: --cc requires an argument");
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
                    eprintln!("m2c: -l requires an argument");
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
                    eprintln!("m2c: -L requires an argument");
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
                    eprintln!("m2c: --cflag requires an argument");
                    process::exit(1);
                }
                opts.extra_cflags.push(args[i].clone());
            }
            "--emit-per-module" => {
                opts.emit_per_module = true;
                opts.emit_c = true;  // per-module implies emit-c (no linking by driver)
            }
            "--out-dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m2c: --out-dir requires an argument");
                    process::exit(1);
                }
                opts.out_dir = Some(PathBuf::from(&args[i]));
            }
            arg if arg.starts_with('-') => {
                eprintln!("m2c: unknown option '{}'", arg);
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
        eprintln!("m2c: no input file");
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
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let env = if os == "linux" { "gnu" } else if os == "macos" { "darwin" } else { "unknown" };
    format!("{}-{}-{}", arch, os, env)
}

fn run_init(args: &[String]) {
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("m2c: cannot determine working directory: {}", e);
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
        eprintln!("m2c: m2.toml already exists in this directory");
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
        eprintln!("m2c: failed to write m2.toml: {}", e);
        process::exit(1);
    });

    // Create src/Main.mod
    let src_dir = cwd.join("src");
    std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
        eprintln!("m2c: failed to create src/: {}", e);
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
        eprintln!("m2c: failed to write src/Main.mod: {}", e);
        process::exit(1);
    });

    // Create tests/Main.mod
    let tests_dir = cwd.join("tests");
    std::fs::create_dir_all(&tests_dir).unwrap_or_else(|e| {
        eprintln!("m2c: failed to create tests/: {}", e);
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
        eprintln!("m2c: failed to write tests/Main.mod: {}", e);
        process::exit(1);
    });

    eprintln!("m2c: initialized project '{}'", name);
    eprintln!("  created m2.toml");
    eprintln!("  created src/Main.mod");
    eprintln!("  created tests/Main.mod");
}

fn run_subcommand(args: &[String]) {
    let subcmd = &args[1];
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("m2c: cannot determine working directory: {}", e);
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
                eprintln!("m2c: no m2.toml found in current or parent directories");
                process::exit(1);
            }
        };
        match build::clean_project(&root) {
            Ok(()) => eprintln!("m2c: cleaned"),
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
    let mut cc = "cc".to_string();
    let mut features: Vec<String> = Vec::new();
    let mut run_args: Vec<String> = Vec::new();
    let mut saw_dashdash = false;

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
            "--cc" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m2c: --cc requires an argument");
                    process::exit(1);
                }
                cc = args[i].clone();
            }
            "--feature" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m2c: --feature requires an argument");
                    process::exit(1);
                }
                features.push(args[i].clone());
            }
            arg if arg.starts_with('-') => {
                eprintln!("m2c {}: unknown option '{}'", subcmd, arg);
                process::exit(1);
            }
            _ => {
                eprintln!("m2c {}: unexpected argument '{}'", subcmd, args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Load project context
    let root = match project_resolver::find_project_root(&cwd) {
        Some(r) => r,
        None => {
            eprintln!("m2c: no m2.toml found in current or parent directories");
            process::exit(1);
        }
    };

    let ctx = match project_resolver::ProjectContext::load(&root, &[]) {
        Some(c) => c,
        None => {
            eprintln!("m2c: failed to load project from {}", root.display());
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
        }
    };

    let config = build::BuildConfig {
        root: root.clone(),
        manifest,
        include_paths,
        cc,
        opt_level: if release { 2 } else { 0 },
        verbose,
        features,
        debug,
    };

    match build::build_project(&config, is_run, &run_args) {
        Ok(result) => {
            if !is_run && !result.up_to_date {
                eprintln!("m2c: built {}", result.artifact.display());
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
