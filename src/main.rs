#![allow(dead_code, unused_imports, unused_variables, unused_parens)]

mod ast;
mod builtins;
mod codegen;
mod driver;
mod errors;
mod json;
mod lexer;
mod lsp;
mod parser;
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
        eprintln!("  --diagnostics-json  Emit errors as JSONL to stderr");
        eprintln!("  --feature <name>    Enable a feature for conditional compilation");
        eprintln!("  --lsp               Start LSP server (JSON-RPC over stdio)");
        eprintln!("  -l <lib>       Link with library");
        eprintln!("  -L <path>      Add library search path");
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
        lsp::run_lsp_server(m2plus, include_paths);
        return;
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
            "--m2plus" => opts.m2plus = true,
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
