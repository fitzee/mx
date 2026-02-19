//! Shared project manifest (m2.toml) and lockfile (m2.lock) parsing.
//!
//! This module lives at the crate root so both the LSP and any future
//! compiler-driver integration can resolve project context without
//! depending on the `lsp` module.

use std::path::{Path, PathBuf};

// ── Manifest ────────────────────────────────────────────────────────

pub struct CcSection {
    pub cflags: Vec<String>,
    pub ldflags: Vec<String>,
    pub libs: Vec<String>,
    pub extra_c: Vec<String>,
    pub frameworks: Vec<String>,
}

impl Default for CcSection {
    fn default() -> Self {
        Self {
            cflags: Vec::new(),
            ldflags: Vec::new(),
            libs: Vec::new(),
            extra_c: Vec::new(),
            frameworks: Vec::new(),
        }
    }
}

pub struct TestSection {
    pub entry: String,
    pub includes: Vec<String>,
}

impl Default for TestSection {
    fn default() -> Self {
        Self {
            entry: "tests/Main.mod".to_string(),
            includes: Vec::new(),
        }
    }
}

pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub m2plus: bool,
    pub includes: Vec<String>,
    pub deps: Vec<DepEntry>,
    pub cc: CcSection,
    pub test: TestSection,
}

pub struct DepEntry {
    pub name: String,
    pub source: DepSource,
}

pub enum DepSource {
    Local(String),
    Registry(String),
}

impl Manifest {
    pub fn parse(content: &str) -> Option<Manifest> {
        let mut name = String::new();
        let mut version = String::new();
        let mut entry = String::new();
        let mut m2plus = false;
        let mut includes = Vec::new();
        let mut deps = Vec::new();
        let mut cc = CcSection::default();
        let mut test = TestSection::default();
        let mut section = "package";

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Section headers
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let sec = &trimmed[1..trimmed.len() - 1];
                section = match sec {
                    "deps" => "deps",
                    "cc" => "cc",
                    "test" => "test",
                    "features" => "features",
                    "registry" => "registry",
                    _ => "other",
                };
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let val = trimmed[eq_pos + 1..].trim();

                match section {
                    "package" => match key {
                        "name" => name = val.to_string(),
                        "version" => version = val.to_string(),
                        "entry" => entry = val.to_string(),
                        "m2plus" => m2plus = val == "true" || val == "1",
                        "includes" => {
                            includes = val.split_whitespace().map(|s| s.to_string()).collect();
                        }
                        "edition" => {
                            if val == "m2plus" {
                                m2plus = true;
                            }
                        }
                        _ => {}
                    },
                    "deps" => {
                        let dep_name = key.to_string();
                        let source = if let Some(path) = val.strip_prefix("path:") {
                            DepSource::Local(path.to_string())
                        } else {
                            DepSource::Registry(val.to_string())
                        };
                        deps.push(DepEntry { name: dep_name, source });
                    }
                    "cc" => match key {
                        "cflags" => cc.cflags = val.split_whitespace().map(|s| s.to_string()).collect(),
                        "ldflags" => cc.ldflags = val.split_whitespace().map(|s| s.to_string()).collect(),
                        "libs" => cc.libs = val.split_whitespace().map(|s| s.to_string()).collect(),
                        "extra-c" => cc.extra_c = val.split_whitespace().map(|s| s.to_string()).collect(),
                        "frameworks" => cc.frameworks = val.split_whitespace().map(|s| s.to_string()).collect(),
                        _ => {}
                    },
                    "test" => match key {
                        "entry" => test.entry = val.to_string(),
                        "includes" => test.includes = val.split_whitespace().map(|s| s.to_string()).collect(),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            return None;
        }

        Some(Manifest { name, version, entry, m2plus, includes, deps, cc, test })
    }
}

// ── Lockfile ────────────────────────────────────────────────────────

pub struct Lockfile {
    pub package_name: String,
    pub package_version: String,
    pub deps: Vec<LockDep>,
}

pub struct LockDep {
    pub name: String,
    pub version: String,
    pub source: String,
    pub sha256: String,
    pub path: String,
}

impl Lockfile {
    pub fn parse(content: &str) -> Option<Lockfile> {
        let mut package_name = String::new();
        let mut package_version = String::new();
        let mut deps: Vec<LockDep> = Vec::new();
        let mut section = "none";
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let sec = &trimmed[1..trimmed.len() - 1];
                if sec == "package" {
                    section = "package";
                } else if let Some(dep_name) = sec.strip_prefix("dep.") {
                    section = "dep";
                    deps.push(LockDep {
                        name: dep_name.to_string(),
                        version: String::new(),
                        source: String::new(),
                        sha256: String::new(),
                        path: String::new(),
                    });
                } else {
                    section = "other";
                }
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let val = trimmed[eq_pos + 1..].trim();

                match section {
                    "package" => match key {
                        "name" => package_name = val.to_string(),
                        "version" => package_version = val.to_string(),
                        _ => {}
                    },
                    "dep" => {
                        if let Some(dep) = deps.last_mut() {
                            match key {
                                "version" => dep.version = val.to_string(),
                                "source" => dep.source = val.to_string(),
                                "sha256" => dep.sha256 = val.to_string(),
                                "path" => dep.path = val.to_string(),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if package_name.is_empty() {
            return None;
        }

        Some(Lockfile { package_name, package_version, deps })
    }

    /// FNV-1a hash of lockfile content for cache invalidation.
    pub fn content_hash(content: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in content.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

// ── ProjectContext ───────────────────────────────────────────────────

pub struct ProjectContext {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub lockfile: Option<Lockfile>,
    pub include_paths: Vec<PathBuf>,
    pub m2plus: bool,
    pub lock_hash: u64,
}

/// Walk up directories from file_path until we find m2.toml.
pub fn find_project_root(file_path: &Path) -> Option<PathBuf> {
    let mut dir = if file_path.is_file() {
        file_path.parent()?.to_path_buf()
    } else {
        file_path.to_path_buf()
    };

    loop {
        let candidate = dir.join("m2.toml");
        if candidate.exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Resolve all include paths: manifest includes, dep includes, CLI fallback.
pub fn resolve_include_paths(
    root: &Path,
    manifest: &Manifest,
    lockfile: Option<&Lockfile>,
    cli_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Manifest includes (relative to project root)
    for inc in &manifest.includes {
        let p = root.join(inc);
        if p.is_dir() {
            paths.push(p);
        }
    }

    // 2. Dep includes from lockfile
    if let Some(lock) = lockfile {
        for dep in &lock.deps {
            if dep.path.is_empty() {
                continue;
            }
            let dep_root = root.join(&dep.path);
            // Try to read the dep's own m2.toml for its includes
            let dep_manifest_path = dep_root.join("m2.toml");
            if let Ok(dep_content) = std::fs::read_to_string(&dep_manifest_path) {
                if let Some(dep_manifest) = Manifest::parse(&dep_content) {
                    for inc in &dep_manifest.includes {
                        let p = dep_root.join(inc);
                        if p.is_dir() {
                            paths.push(p);
                        }
                    }
                    continue;
                }
            }
            // Fallback: dep_root/src if it exists
            let src_dir = dep_root.join("src");
            if src_dir.is_dir() {
                paths.push(src_dir);
            }
        }
    }

    // 3. CLI fallback paths (append any not already present)
    for cli in cli_paths {
        if !paths.contains(cli) {
            paths.push(cli.clone());
        }
    }

    paths
}

impl ProjectContext {
    pub fn load(root: &Path, cli_paths: &[PathBuf]) -> Option<ProjectContext> {
        let manifest_path = root.join("m2.toml");
        let manifest_content = std::fs::read_to_string(&manifest_path).ok()?;
        let manifest = Manifest::parse(&manifest_content)?;
        let m2plus = manifest.m2plus;

        let lock_path = root.join("m2.lock");
        let (lockfile, lock_hash) = if let Ok(lock_content) = std::fs::read_to_string(&lock_path) {
            let hash = Lockfile::content_hash(&lock_content);
            (Lockfile::parse(&lock_content), hash)
        } else {
            (None, 0)
        };

        let include_paths = resolve_include_paths(root, &manifest, lockfile.as_ref(), cli_paths);

        Some(ProjectContext {
            root: root.to_path_buf(),
            manifest,
            lockfile,
            include_paths,
            m2plus,
            lock_hash,
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest_basic() {
        let content = "\
# m2.toml - test manifest
name=myproject
version=1.0.0
entry=src/Main.mod
m2plus=true
includes=src lib
";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.name, "myproject");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.entry, "src/Main.mod");
        assert!(m.m2plus);
        assert_eq!(m.includes, vec!["src", "lib"]);
        assert!(m.deps.is_empty());
    }

    #[test]
    fn test_parse_manifest_deps() {
        let content = "\
name=app
version=0.1.0
entry=src/Main.mod

[deps]
mylib=path:../mylib
otherlib=0.2.0
";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.deps.len(), 2);
        assert_eq!(m.deps[0].name, "mylib");
        assert!(matches!(&m.deps[0].source, DepSource::Local(p) if p == "../mylib"));
        assert_eq!(m.deps[1].name, "otherlib");
        assert!(matches!(&m.deps[1].source, DepSource::Registry(v) if v == "0.2.0"));
    }

    #[test]
    fn test_parse_manifest_edition_m2plus() {
        let content = "\
name=app
version=0.1.0
entry=src/Main.mod
edition=m2plus
";
        let m = Manifest::parse(content).unwrap();
        assert!(m.m2plus);
    }

    #[test]
    fn test_parse_manifest_cc_section() {
        let content = "\
name=myapp
version=1.0.0
entry=src/Main.mod

[cc]
cflags=-Wall -Wextra
ldflags=-L/usr/local/lib -L/opt/lib
libs=m pthread
extra-c=runtime.c helper.c
frameworks=CoreFoundation IOKit
";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.cc.cflags, vec!["-Wall", "-Wextra"]);
        assert_eq!(m.cc.ldflags, vec!["-L/usr/local/lib", "-L/opt/lib"]);
        assert_eq!(m.cc.libs, vec!["m", "pthread"]);
        assert_eq!(m.cc.extra_c, vec!["runtime.c", "helper.c"]);
        assert_eq!(m.cc.frameworks, vec!["CoreFoundation", "IOKit"]);
    }

    #[test]
    fn test_parse_manifest_test_section() {
        let content = "\
name=myapp
version=1.0.0

[test]
entry=tests/TestMain.mod
includes=tests tests/fixtures
";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.test.entry, "tests/TestMain.mod");
        assert_eq!(m.test.includes, vec!["tests", "tests/fixtures"]);
    }

    #[test]
    fn test_parse_manifest_test_defaults() {
        let content = "name=myapp\nversion=1.0.0\n";
        let m = Manifest::parse(content).unwrap();
        assert_eq!(m.test.entry, "tests/Main.mod");
        assert!(m.test.includes.is_empty());
        assert!(m.cc.cflags.is_empty());
    }

    #[test]
    fn test_parse_manifest_empty_name() {
        let content = "version=0.1.0\n";
        assert!(Manifest::parse(content).is_none());
    }

    #[test]
    fn test_parse_lockfile() {
        let content = "\
# m2.lock - generated by m2pkg resolve

[package]
name=myapp
version=1.0.0

[dep.mylib]
version=0.3.0
source=local
sha256=abc123
path=../mylib

[dep.utils]
version=1.2.0
source=registry
sha256=def456
path=.m2pkg/cache/utils-1.2.0
";
        let l = Lockfile::parse(content).unwrap();
        assert_eq!(l.package_name, "myapp");
        assert_eq!(l.package_version, "1.0.0");
        assert_eq!(l.deps.len(), 2);
        assert_eq!(l.deps[0].name, "mylib");
        assert_eq!(l.deps[0].version, "0.3.0");
        assert_eq!(l.deps[0].source, "local");
        assert_eq!(l.deps[0].sha256, "abc123");
        assert_eq!(l.deps[0].path, "../mylib");
        assert_eq!(l.deps[1].name, "utils");
        assert_eq!(l.deps[1].version, "1.2.0");
        assert_eq!(l.deps[1].source, "registry");
    }

    #[test]
    fn test_parse_lockfile_empty_name() {
        let content = "[package]\nversion=0.1.0\n";
        assert!(Lockfile::parse(content).is_none());
    }

    #[test]
    fn test_find_project_root() {
        // Create a temp directory structure
        let tmp = std::env::temp_dir().join("m2_resolver_test_workspace");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).unwrap();
        std::fs::write(tmp.join("m2.toml"), "name=test\nversion=0.1.0\n").unwrap();
        std::fs::write(tmp.join("src/Main.mod"), "MODULE Main; END Main.").unwrap();

        let found = find_project_root(&tmp.join("src/Main.mod"));
        assert_eq!(found.unwrap(), tmp);

        // From the root itself
        let found2 = find_project_root(&tmp.join("m2.toml"));
        assert_eq!(found2.unwrap(), tmp);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_lockfile_hash_changes() {
        let content1 = "[package]\nname=a\nversion=0.1.0\n";
        let content2 = "[package]\nname=a\nversion=0.2.0\n";
        let h1 = Lockfile::content_hash(content1);
        let h2 = Lockfile::content_hash(content2);
        assert_ne!(h1, h2);

        // Same content → same hash
        let h3 = Lockfile::content_hash(content1);
        assert_eq!(h1, h3);
    }
}
