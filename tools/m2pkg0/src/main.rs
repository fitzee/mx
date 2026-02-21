/// m2pkg0 — minimal bootstrapper for m2pkg
///
/// Builds m2pkg from its Modula-2 sources using m2c, then optionally runs it.
/// Usage:
///   m2pkg0 build              Build m2pkg from tools/m2pkg/
///   m2pkg0 run [args...]      Build and run m2pkg with the given arguments

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn find_project_root() -> Option<PathBuf> {
    // Walk up from the current exe or cwd looking for Cargo.toml + tools/m2pkg/
    let mut dir = env::current_dir().ok()?;
    for _ in 0..10 {
        if dir.join("tools/m2pkg/m2.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn find_m2c() -> Option<PathBuf> {
    // Try development build first, then system PATH
    if let Some(root) = find_project_root() {
        let dev = root.join("target/debug/m2c");
        if dev.exists() {
            return Some(dev);
        }
        let rel = root.join("target/release/m2c");
        if rel.exists() {
            return Some(rel);
        }
    }
    // Fall back to PATH
    which("m2c")
}

fn which(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.exists())
    })
}

fn build_m2pkg(root: &Path, m2c: &Path) -> Result<PathBuf, String> {
    let pkg_dir = root.join("tools/m2pkg");
    let sys_c = root.join("libs/m2sys/m2sys.c");
    let out_dir = pkg_dir.join("target");

    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {}", out_dir.display(), e))?;

    let output = out_dir.join("m2pkg");

    // Read m2.toml to determine entry and extra-c
    let manifest = std::fs::read_to_string(pkg_dir.join("m2.toml"))
        .map_err(|e| format!("cannot read m2.toml: {}", e))?;

    let mut entry = String::from("src/Main.mod");
    let mut includes = Vec::new();
    let mut extra_c = Vec::new();
    let mut m2plus = false;

    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            match key.trim() {
                "entry" => entry = val.trim().to_string(),
                "includes" => {
                    for inc in val.trim().split_whitespace() {
                        includes.push(inc.to_string());
                    }
                }
                "extra-c" => extra_c.push(val.trim().to_string()),
                "edition" => {
                    if val.trim() == "m2plus" {
                        m2plus = true;
                    }
                }
                "m2plus" => {
                    if val.trim() == "true" || val.trim() == "1" {
                        m2plus = true;
                    }
                }
                _ => {}
            }
        }
    }

    let mut cmd = Command::new(m2c);
    if m2plus {
        cmd.arg("--m2plus");
    }
    for inc in &includes {
        cmd.arg("-I").arg(inc);
    }
    cmd.arg(&entry);

    // Resolve extra-c paths relative to pkg_dir
    for ec in &extra_c {
        let ec_path = pkg_dir.join(ec);
        if ec_path.exists() {
            cmd.arg(&ec_path);
        } else if sys_c.exists() && ec.contains("m2sys") {
            cmd.arg(&sys_c);
        } else {
            return Err(format!("extra-c file not found: {}", ec));
        }
    }

    cmd.arg("-o").arg(&output);
    cmd.current_dir(&pkg_dir);

    eprintln!(
        "m2pkg0: {} {}",
        m2c.display(),
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    );

    let status = cmd.status().map_err(|e| format!("failed to run m2c: {}", e))?;
    if !status.success() {
        return Err(format!(
            "m2c exited with {}",
            status.code().unwrap_or(-1)
        ));
    }

    eprintln!("m2pkg0: built {}", output.display());
    Ok(output)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        eprintln!("m2pkg0 — bootstrap builder for m2pkg");
        eprintln!("Usage:");
        eprintln!("  m2pkg0 build          Build m2pkg from source");
        eprintln!("  m2pkg0 run [args...]   Build and run m2pkg");
        return ExitCode::from(1);
    }

    let root = match find_project_root() {
        Some(r) => r,
        None => {
            eprintln!("m2pkg0: cannot find project root (no tools/m2pkg/m2.toml found)");
            return ExitCode::from(1);
        }
    };

    let m2c = match find_m2c() {
        Some(p) => p,
        None => {
            eprintln!("m2pkg0: cannot find m2c compiler");
            eprintln!("  hint: run `cargo build` in the project root first");
            return ExitCode::from(1);
        }
    };

    match args[1].as_str() {
        "build" => match build_m2pkg(&root, &m2c) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("m2pkg0: {}", e);
                ExitCode::from(1)
            }
        },
        "run" => {
            let m2pkg = match build_m2pkg(&root, &m2c) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("m2pkg0: {}", e);
                    return ExitCode::from(1);
                }
            };
            let status = Command::new(&m2pkg)
                .args(&args[2..])
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("m2pkg0: failed to run m2pkg: {}", e);
                    std::process::exit(1);
                });
            if status.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(status.code().unwrap_or(1) as u8)
            }
        }
        other => {
            eprintln!("m2pkg0: unknown command '{}' (try 'build' or 'run')", other);
            ExitCode::from(1)
        }
    }
}
