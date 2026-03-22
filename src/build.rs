//! Project build engine for `build`/`run`/`test`/`clean` subcommands.
//!
//! Uses stamp-based skip: if no source file changed since the last build,
//! the entire compilation is skipped.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::driver::CompileOptions;
use crate::errors::{CompileError, CompileResult};
use crate::identity;
use crate::project_resolver::Manifest;

// ── FileStamp ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub mtime_secs: u64,
    pub size: u64,
    pub hash: u64,
}

impl FileStamp {
    /// Read stamp from a file on disk.
    pub fn from_path(p: &Path) -> Option<FileStamp> {
        let meta = fs::metadata(p).ok()?;
        let mtime_secs = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let size = meta.len();
        let content = fs::read(p).ok()?;
        let hash = fnv1a(&content);
        Some(FileStamp { mtime_secs, size, hash })
    }
}

fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ── BuildState ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct BuildState {
    pub stamps: HashMap<String, FileStamp>,
    pub last_build_hash: u64,
}

impl BuildState {
    /// Load from `{BUILD_DIR}/build_state.json`, or return empty state.
    pub fn load(root: &Path) -> BuildState {
        let state_path = root.join(format!("{}/build_state.json", identity::BUILD_DIR));
        let content = match fs::read_to_string(&state_path) {
            Ok(c) => c,
            Err(_) => return BuildState { stamps: HashMap::new(), last_build_hash: 0 },
        };

        let json = match crate::json::Json::parse(&content) {
            Ok(j) => j,
            Err(_) => return BuildState { stamps: HashMap::new(), last_build_hash: 0 },
        };

        let mut stamps = HashMap::new();
        if let Some(stamps_obj) = json.get("stamps") {
            if let Some(entries) = stamps_obj.as_object() {
                for (key, val) in entries {
                    let mtime = val.get("mtime").and_then(|v| v.as_i64()).unwrap_or(0) as u64;
                    let size = val.get("size").and_then(|v| v.as_i64()).unwrap_or(0) as u64;
                    let hash = val.get("hash").and_then(|v| v.as_str())
                        .and_then(|s| u64::from_str_radix(s, 16).ok())
                        .unwrap_or(0);
                    stamps.insert(key.clone(), FileStamp { mtime_secs: mtime, size, hash });
                }
            }
        }
        let last_build_hash = json.get("last_build_hash").and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s, 16).ok())
            .unwrap_or(0);

        BuildState { stamps, last_build_hash }
    }

    /// Save to `{BUILD_DIR}/build_state.json`.
    pub fn save(&self, root: &Path) -> CompileResult<()> {
        let dir = root.join(identity::BUILD_DIR);
        fs::create_dir_all(&dir).map_err(|e| {
            CompileError::driver(format!("cannot create {}/: {}", identity::BUILD_DIR, e))
        })?;

        let mut stamp_entries = Vec::new();
        let mut keys: Vec<&String> = self.stamps.keys().collect();
        keys.sort();
        for key in keys {
            let s = &self.stamps[key];
            let entry = crate::json::Json::obj(vec![
                ("mtime", crate::json::Json::int_val(s.mtime_secs as i64)),
                ("size", crate::json::Json::int_val(s.size as i64)),
                ("hash", crate::json::Json::str_val(&format!("{:x}", s.hash))),
            ]);
            stamp_entries.push((key.as_str(), entry));
        }

        let root_obj = crate::json::Json::obj(vec![
            ("last_build_hash", crate::json::Json::str_val(&format!("{:x}", self.last_build_hash))),
            ("stamps", crate::json::Json::obj(stamp_entries)),
        ]);

        let state_path = dir.join("build_state.json");
        fs::write(&state_path, root_obj.serialize()).map_err(|e| {
            CompileError::driver(format!("cannot write build_state.json: {}", e))
        })
    }

    /// Check whether a file's stamp differs from the stored one.
    pub fn is_stale(&self, path: &str, current: &FileStamp) -> bool {
        match self.stamps.get(path) {
            None => true,
            Some(stored) => stored != current,
        }
    }
}

// ── BuildConfig ─────────────────────────────────────────────────────

pub struct BuildConfig {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub include_paths: Vec<PathBuf>,
    pub cc: String,
    pub opt_level: u8,
    pub verbose: bool,
    pub features: Vec<String>,
    pub debug: bool,
    pub use_llvm: bool,
}

// ── build_project ───────────────────────────────────────────────────

pub struct BuildResult {
    pub artifact: PathBuf,
    pub up_to_date: bool,
}

/// Main build entry point. Returns path to built artifact and whether it was up-to-date.
pub fn build_project(
    config: &BuildConfig,
    run_after: bool,
    run_args: &[String],
) -> CompileResult<BuildResult> {
    let root = &config.root;
    let manifest = &config.manifest;

    // Resolve entry point
    let entry = if manifest.entry.is_empty() {
        "src/Main.mod".to_string()
    } else {
        manifest.entry.clone()
    };
    let entry_path = root.join(&entry);
    if !entry_path.exists() {
        return Err(CompileError::driver(format!(
            "entry file not found: {}",
            entry_path.display()
        )));
    }

    // Collect source files to stamp
    let mut source_files: Vec<PathBuf> = Vec::new();
    source_files.push(entry_path.clone());

    // Scan include paths for .mod/.def files
    for inc in &config.include_paths {
        if inc.is_dir() {
            collect_source_files(inc, &mut source_files);
        }
    }

    // Also stamp the manifest and lockfile
    let manifest_path = root.join("m2.toml");
    if manifest_path.exists() {
        source_files.push(manifest_path);
    }
    let lock_path = root.join("m2.lock");
    if lock_path.exists() {
        source_files.push(lock_path);
    }

    // Stamp the compiler binary itself so rebuilding the compiler invalidates caches
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            source_files.push(canonical);
        }
    }

    // Build current stamps
    let mut current_stamps: HashMap<String, FileStamp> = HashMap::new();
    for path in &source_files {
        if let Some(stamp) = FileStamp::from_path(path) {
            let rel = path.strip_prefix(root).unwrap_or(path);
            current_stamps.insert(rel.to_string_lossy().to_string(), stamp);
        }
    }

    // Compute combined hash (includes content hash + mtime + size for each file)
    let mut combined: u64 = 0xcbf29ce484222325;
    let mut keys: Vec<&String> = current_stamps.keys().collect();
    keys.sort();
    for key in &keys {
        let stamp = &current_stamps[*key];
        combined ^= stamp.hash;
        combined = combined.wrapping_mul(0x100000001b3);
        combined ^= stamp.mtime_secs;
        combined = combined.wrapping_mul(0x100000001b3);
        combined ^= stamp.size;
        combined = combined.wrapping_mul(0x100000001b3);
    }

    // Load previous state and check staleness
    let prev_state = BuildState::load(root);
    if prev_state.last_build_hash == combined && combined != 0 {
        let artifact = artifact_path(root, &manifest.name);
        if artifact.exists() {
            eprintln!("{}: up to date", identity::COMPILER_NAME);
            if run_after {
                run_artifact(&artifact, run_args)?;
            }
            return Ok(BuildResult { artifact, up_to_date: true });
        }
    }

    // Ensure output directories
    let bin_dir = root.join(format!("{}/bin", identity::BUILD_DIR));
    let gen_dir = root.join(format!("{}/gen", identity::BUILD_DIR));
    fs::create_dir_all(&bin_dir).map_err(|e| {
        CompileError::driver(format!("cannot create {}/bin/: {}", identity::BUILD_DIR, e))
    })?;
    fs::create_dir_all(&gen_dir).map_err(|e| {
        CompileError::driver(format!("cannot create {}/gen/: {}", identity::BUILD_DIR, e))
    })?;

    // Build CompileOptions
    let artifact = artifact_path(root, &manifest.name);
    let c_output = gen_dir.join(format!("{}.c", manifest.name));

    let mut opts = CompileOptions::default();
    opts.input = entry_path;
    opts.output = Some(artifact.clone());
    opts.include_paths = config.include_paths.clone();
    opts.m2plus = manifest.m2plus;
    opts.cc = config.cc.clone();
    opts.opt_level = config.opt_level;
    opts.verbose = config.verbose;
    opts.debug = config.debug;
    opts.emit_llvm = config.use_llvm;
    opts.use_llvm = config.use_llvm;

    // Auto-inject platform feature (MACOS or LINUX)
    let mut features = config.features.clone();
    if cfg!(target_os = "macos") {
        if !features.contains(&"MACOS".to_string()) {
            features.push("MACOS".to_string());
        }
    } else if cfg!(target_os = "linux") {
        if !features.contains(&"LINUX".to_string()) {
            features.push("LINUX".to_string());
        }
    }
    opts.features = features.clone();

    // Apply manifest [cc] section, merged with any active feature-gated [cc] sections
    let effective_cc = manifest.merged_cc(&features);
    opts.extra_cflags = effective_cc.cflags.clone();
    opts.frameworks = effective_cc.frameworks.clone();
    for extra in &effective_cc.extra_c {
        opts.extra_c_files.push(root.join(extra));
    }
    for lib in &effective_cc.libs {
        opts.link_libs.push(lib.clone());
    }
    for ldflag in &effective_cc.ldflags {
        // Extract -L paths or pass through
        if let Some(path) = ldflag.strip_prefix("-L") {
            opts.link_paths.push(path.to_string());
        } else {
            opts.link_paths.push(ldflag.clone());
        }
    }

    // Canonicalize existing extra_c paths for dedup
    let canonical_extras: Vec<PathBuf> = opts.extra_c_files.iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();

    // Load lockfile for transitive dep resolution
    let lockfile = std::fs::read_to_string(root.join("m2.lock"))
        .ok()
        .and_then(|c| crate::project_resolver::Lockfile::parse(&c));

    // Merge transitive [cc] sections from dependencies
    let dep_cc = crate::project_resolver::collect_transitive_cc(root, manifest, lockfile.as_ref(), &features);
    for f in &dep_cc.cflags {
        if !opts.extra_cflags.contains(f) {
            opts.extra_cflags.push(f.clone());
        }
    }
    for f in &dep_cc.frameworks {
        if !opts.frameworks.contains(f) {
            opts.frameworks.push(f.clone());
        }
    }
    for extra in &dep_cc.extra_c {
        // Canonicalize to deduplicate paths that resolve to the same file
        let p = PathBuf::from(extra);
        let dominated = p.canonicalize()
            .map(|cp| canonical_extras.contains(&cp))
            .unwrap_or(false);
        if !dominated && !opts.extra_c_files.contains(&p) {
            opts.extra_c_files.push(p);
        }
    }
    for lib in &dep_cc.libs {
        if !opts.link_libs.contains(lib) {
            opts.link_libs.push(lib.clone());
        }
    }
    for ldflag in &dep_cc.ldflags {
        if let Some(path) = ldflag.strip_prefix("-L") {
            if !opts.link_paths.contains(&path.to_string()) {
                opts.link_paths.push(path.to_string());
            }
        } else if !opts.link_paths.contains(ldflag) {
            opts.link_paths.push(ldflag.clone());
        }
    }

    if config.verbose {
        eprintln!("{}: building {} → {}", identity::COMPILER_NAME, entry, artifact.display());
    }

    crate::driver::compile(&opts)?;

    // Save build state
    let new_state = BuildState {
        stamps: current_stamps,
        last_build_hash: combined,
    };
    new_state.save(root)?;

    if config.verbose {
        eprintln!("{}: built {}", identity::COMPILER_NAME, artifact.display());
    }

    if run_after {
        run_artifact(&artifact, run_args)?;
    }

    Ok(BuildResult { artifact, up_to_date: false })
}

/// Remove the build directory.
pub fn clean_project(root: &Path) -> CompileResult<()> {
    let build_dir = root.join(identity::BUILD_DIR);
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).map_err(|e| {
            CompileError::driver(format!("cannot remove {}/: {}", identity::BUILD_DIR, e))
        })?;
    }
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────

fn artifact_path(root: &Path, name: &str) -> PathBuf {
    root.join(format!("{}/bin", identity::BUILD_DIR)).join(name)
}

fn collect_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "mod" | "MOD" | "def" | "DEF" => {
                    if !out.contains(&path) {
                        out.push(path);
                    }
                }
                _ => {}
            }
        }
    }
}

fn run_artifact(artifact: &Path, args: &[String]) -> CompileResult<()> {
    let status = std::process::Command::new(artifact)
        .args(args)
        .status()
        .map_err(|e| {
            CompileError::driver(format!("cannot run '{}': {}", artifact.display(), e))
        })?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a_basic() {
        let h1 = fnv1a(b"hello");
        let h2 = fnv1a(b"hello");
        assert_eq!(h1, h2);

        let h3 = fnv1a(b"world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_file_stamp_from_path() {
        let tmp = std::env::temp_dir().join("m2_build_test_stamp");
        let _ = fs::remove_file(&tmp);
        fs::write(&tmp, "test content").unwrap();
        let stamp = FileStamp::from_path(&tmp).unwrap();
        assert!(stamp.size > 0);
        assert!(stamp.hash != 0);

        // Same content → same hash
        let stamp2 = FileStamp::from_path(&tmp).unwrap();
        assert_eq!(stamp.hash, stamp2.hash);

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_build_state_stale() {
        let state = BuildState {
            stamps: HashMap::new(),
            last_build_hash: 0,
        };
        let stamp = FileStamp { mtime_secs: 100, size: 50, hash: 12345 };
        assert!(state.is_stale("foo.mod", &stamp));

        let mut state2 = BuildState {
            stamps: HashMap::new(),
            last_build_hash: 0,
        };
        state2.stamps.insert("foo.mod".to_string(), stamp.clone());
        assert!(!state2.is_stale("foo.mod", &stamp));

        let changed = FileStamp { mtime_secs: 200, size: 60, hash: 99999 };
        assert!(state2.is_stale("foo.mod", &changed));
    }

    #[test]
    fn test_build_state_save_load() {
        let tmp = std::env::temp_dir().join("m2_build_test_state");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let mut stamps = HashMap::new();
        // Use a large u64 hash that exceeds i64::MAX to test hex round-trip
        stamps.insert("src/Main.mod".to_string(), FileStamp {
            mtime_secs: 1000,
            size: 42,
            hash: 0xEF23456789ABCDEF,
        });

        let state = BuildState { stamps, last_build_hash: 0xCAFEBABE12345678 };
        state.save(&tmp).unwrap();

        let loaded = BuildState::load(&tmp);
        assert_eq!(loaded.last_build_hash, 0xCAFEBABE12345678);
        assert!(loaded.stamps.contains_key("src/Main.mod"));
        let s = &loaded.stamps["src/Main.mod"];
        assert_eq!(s.mtime_secs, 1000);
        assert_eq!(s.size, 42);
        assert_eq!(s.hash, 0xEF23456789ABCDEF);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_clean_project() {
        let tmp = std::env::temp_dir().join("m2_build_test_clean");
        let _ = fs::remove_dir_all(&tmp);
        let build_dir = tmp.join(identity::BUILD_DIR);
        fs::create_dir_all(build_dir.join("bin")).unwrap();
        fs::write(build_dir.join("build_state.json"), "{}").unwrap();

        assert!(build_dir.exists());
        clean_project(&tmp).unwrap();
        assert!(!build_dir.exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
