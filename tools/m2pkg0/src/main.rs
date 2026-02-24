/// m2pkg0 — minimal bootstrapper for m2pkg
///
/// Builds m2pkg from its Modula-2 sources using m2c, then optionally runs it.
/// Usage:
///   m2pkg0 build              Build m2pkg from tools/m2pkg/
///   m2pkg0 run [args...]      Build and run m2pkg with the given arguments

use std::collections::HashSet;
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

/// Parsed fields from an m2.toml manifest
struct Manifest {
    entry: String,
    includes: Vec<String>,
    extra_c: Vec<String>,
    m2plus: bool,
    /// [deps] section: name -> value (e.g. "path:../m2http")
    deps: Vec<(String, String)>,
    /// [cc] section
    cc_extra_c: Vec<String>,
    cc_libs: Vec<String>,
    cc_cflags: Vec<String>,
    cc_ldflags: Vec<String>,
}

fn parse_manifest(path: &Path) -> Result<Manifest, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;

    let mut m = Manifest {
        entry: String::from("src/Main.mod"),
        includes: Vec::new(),
        extra_c: Vec::new(),
        m2plus: false,
        deps: Vec::new(),
        cc_extra_c: Vec::new(),
        cc_libs: Vec::new(),
        cc_cflags: Vec::new(),
        cc_ldflags: Vec::new(),
    };

    let mut section = String::new(); // "", "deps", "cc"

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        // Section headers
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match section.as_str() {
                "" => match key {
                    "entry" => m.entry = val.to_string(),
                    "includes" => {
                        for inc in val.split_whitespace() {
                            m.includes.push(inc.to_string());
                        }
                    }
                    "extra-c" => m.extra_c.push(val.to_string()),
                    "edition" => {
                        if val == "m2plus" {
                            m.m2plus = true;
                        }
                    }
                    "m2plus" => {
                        if val == "true" || val == "1" {
                            m.m2plus = true;
                        }
                    }
                    _ => {}
                },
                "deps" => {
                    if !val.is_empty() {
                        m.deps.push((key.to_string(), val.to_string()));
                    }
                }
                "cc" => match key {
                    "extra-c" => m.cc_extra_c.push(val.to_string()),
                    "libs" => {
                        for lib in val.split_whitespace() {
                            m.cc_libs.push(lib.to_string());
                        }
                    }
                    "cflags" => {
                        for f in val.split_whitespace() {
                            m.cc_cflags.push(f.to_string());
                        }
                    }
                    "ldflags" => {
                        for f in val.split_whitespace() {
                            m.cc_ldflags.push(f.to_string());
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Ok(m)
}

/// Recursively resolve local path deps from a manifest.
/// Collects include dirs, extra-c files, and link flags.
fn resolve_deps(
    manifest_dir: &Path,
    deps: &[(String, String)],
    visited: &mut HashSet<String>,
    include_dirs: &mut Vec<PathBuf>,
    extra_c_files: &mut Vec<PathBuf>,
    link_flags: &mut Vec<String>,
) -> Result<(), String> {
    for (name, spec) in deps {
        if visited.contains(name) {
            continue;
        }
        visited.insert(name.clone());

        if let Some(rel_path) = spec.strip_prefix("path:") {
            let dep_dir = manifest_dir.join(rel_path);
            let dep_dir = dep_dir.canonicalize().unwrap_or(dep_dir);

            // Add dep's src/ as include dir
            let src_dir = dep_dir.join("src");
            if src_dir.is_dir() && !include_dirs.contains(&src_dir) {
                include_dirs.push(src_dir);
            }

            // Read dep's m2.toml for transitive deps and [cc]
            let dep_toml = dep_dir.join("m2.toml");
            if dep_toml.exists() {
                let dep_manifest = parse_manifest(&dep_toml)?;

                // Collect [cc] extra-c (resolved relative to dep dir)
                for ec in &dep_manifest.cc_extra_c {
                    let ec_path = dep_dir.join(ec);
                    let ec_path = ec_path.canonicalize().unwrap_or(ec_path);
                    if ec_path.exists() && !extra_c_files.contains(&ec_path) {
                        extra_c_files.push(ec_path);
                    }
                }

                // Collect [cc] libs
                for lib in &dep_manifest.cc_libs {
                    if !link_flags.contains(lib) {
                        link_flags.push(lib.clone());
                    }
                }

                // Recurse into transitive deps
                resolve_deps(
                    &dep_dir,
                    &dep_manifest.deps,
                    visited,
                    include_dirs,
                    extra_c_files,
                    link_flags,
                )?;
            }
        }
        // Skip non-path deps (URL, registry) — bootstrapper only handles local
    }
    Ok(())
}

fn build_m2pkg(root: &Path, m2c: &Path) -> Result<PathBuf, String> {
    let pkg_dir = root.join("tools/m2pkg");
    let sys_c = root.join("libs/m2sys/m2sys.c");
    let out_dir = pkg_dir.join("target");

    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {}", out_dir.display(), e))?;

    let output = out_dir.join("m2pkg");

    let manifest = parse_manifest(&pkg_dir.join("m2.toml"))?;

    // Resolve dependencies recursively
    let mut dep_includes = Vec::new();
    let mut dep_extra_c = Vec::new();
    let mut dep_link_flags = Vec::new();
    let mut visited = HashSet::new();
    resolve_deps(
        &pkg_dir,
        &manifest.deps,
        &mut visited,
        &mut dep_includes,
        &mut dep_extra_c,
        &mut dep_link_flags,
    )?;

    let mut cmd = Command::new(m2c);
    if manifest.m2plus {
        cmd.arg("--m2plus");
    }

    // Package's own includes
    for inc in &manifest.includes {
        cmd.arg("-I").arg(inc);
    }

    // Dep include dirs (absolute paths)
    for inc in &dep_includes {
        cmd.arg("-I").arg(inc);
    }

    cmd.arg(&manifest.entry);

    // Package's own extra-c (resolved relative to pkg_dir)
    let mut all_extra_c: Vec<PathBuf> = Vec::new();
    for ec in &manifest.extra_c {
        let ec_path = pkg_dir.join(ec);
        let ec_path = ec_path.canonicalize().unwrap_or(ec_path);
        if ec_path.exists() {
            if !all_extra_c.contains(&ec_path) {
                all_extra_c.push(ec_path);
            }
        } else if sys_c.exists() && ec.contains("m2sys") {
            let sc = sys_c.canonicalize().unwrap_or(sys_c.clone());
            if !all_extra_c.contains(&sc) {
                all_extra_c.push(sc);
            }
        } else {
            return Err(format!("extra-c file not found: {}", ec));
        }
    }

    // Dep extra-c files (deduplicated with package extra-c)
    for ec in &dep_extra_c {
        let ec_canon = ec.canonicalize().unwrap_or(ec.clone());
        if !all_extra_c.contains(&ec_canon) {
            all_extra_c.push(ec_canon);
        }
    }

    // Emit all extra-c files
    for ec in &all_extra_c {
        cmd.arg(ec);
    }

    // Link flags from deps
    for flag in &dep_link_flags {
        cmd.arg(flag);
    }

    // Package's own [cc] cflags (passed as --cflag to m2c)
    for f in &manifest.cc_cflags {
        cmd.arg("--cflag").arg(f);
    }

    // Package's own [cc] ldflags
    for f in &manifest.cc_ldflags {
        cmd.arg(f);
    }

    // Package's own [cc] libs
    for lib in &manifest.cc_libs {
        cmd.arg(lib);
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
