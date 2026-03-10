# Build System Reference

How `mx build` works, how `m2.toml` is structured, and how transitive dependencies are resolved.

---

## Commands

```bash
mx build          # Compile project per m2.toml -> outputs binary
mx run            # Compile + execute
mx test           # Compile + run test entry point
mx clean          # Remove .mx/ build directory and binary
mx init myproject # Create new project scaffold with m2.toml
```

Single-file compilation (no m2.toml needed):

```bash
mx hello.mod -o hello        # Compile single file
mx hello.mod -c              # Emit C only (no linking)
mx hello.mod -g -o hello     # Debug build with DWARF + #line
```

---

## m2.toml Manifest Format

The manifest is a flat key=value file with `[section]` headers. **Not TOML** -- it is a simpler format (no nested tables, no quoted strings, no arrays beyond space-separated values).

### Top-level keys (package section)

These go before any `[section]` header:

| Key | Required | Description | Example |
|-----|----------|-------------|---------|
| `name` | yes | Package name | `name=myapp` |
| `version` | no | Semver version | `version=1.0.0` |
| `entry` | yes | Main program module (relative to project root) | `entry=src/Main.mod` |
| `includes` | no | Space-separated include directories for .def resolution | `includes=src lib` |
| `m2plus` | no | Enable Modula-2+ extensions (true/false) | `m2plus=true` |
| `edition` | no | `pim4` (default) or `m2plus` (implies m2plus=true) | `edition=pim4` |
| `manifest_version` | no | Manifest format version | `manifest_version=1` |

### [deps] section

Declares library dependencies. Three formats:

```toml
[deps]
m2bytes                      # Installed: resolved from ~/.mx/lib/m2bytes/
locallib=path:../locallib    # Local: relative path to library root
remotelib=0.2.0              # Registry: fetched by mxpkg
```

**Bare name** (no `=`): looks for library in `~/.mx/lib/<name>/` (or `$MX_HOME/lib/<name>/`).

**path:**: relative path from the m2.toml directory to the dependency root.

**version string**: fetched from registry by the package manager.

### [cc] section

C compiler flags. All fields are space-separated values. All are optional.

```toml
[cc]
cflags=-I/opt/homebrew/include
ldflags=-L/opt/homebrew/lib
libs=-lssl -lcrypto
extra-c=src/bridge.c src/ffi.c
frameworks=CoreFoundation Security
```

| Field | Purpose |
|-------|---------|
| `cflags` | Passed to cc during compilation |
| `ldflags` | Passed to cc during linking |
| `libs` | Libraries to link (with or without `-l` prefix) |
| `extra-c` | Additional C source files (paths relative to project root) |
| `frameworks` | macOS `-framework` flags |

**Transitive propagation:** When A depends on B, and B has `[cc]` settings, those settings automatically propagate to A's compilation. This means a library like `m2tls` can declare `libs=-lssl -lcrypto` and any project that depends on it will automatically link OpenSSL.

### [cc.feature.NAME] section

Feature-gated CC overrides. Only applied when the feature is active.

```toml
[features]
use_tls

[cc.feature.use_tls]
libs=-lssl -lcrypto
```

Fields are merged with the base `[cc]` section when `--feature use_tls` is passed (or when mxpkg activates the feature). Duplicate values are deduplicated.

### [test] section

```toml
[test]
entry=tests/Main.mod
includes=tests
```

| Field | Default | Description |
|-------|---------|-------------|
| `entry` | `tests/Main.mod` | Test program module |
| `includes` | (empty) | Additional include paths for test compilation |

### [features] section

Feature gate declarations. Each line is a feature name (no values).

```toml
[features]
use_tls
debug_logging
```

Activate with `--feature <name>` on the CLI or via mxpkg.

### [registry] section

```toml
[registry]
url=https://registry.example.com
```

---

## Transitive Dependency Resolution

When `mx build` runs:

1. Parses project's `m2.toml`
2. For each dependency in `[deps]`:
   - Resolves the dependency root (installed path, local path, or registry path)
   - Reads the dependency's `m2.toml`
   - Adds the dependency's `includes` paths to the compiler's `-I` search paths
   - Reads the dependency's `[cc]` section
3. Recursively resolves each dependency's own `[deps]`
4. Collects all transitive `[cc]` settings (cflags, ldflags, libs, extra-c, frameworks) and merges them
5. Deduplicates flags and paths (canonicalized to prevent duplicates from symlinks)

### Include path order

1. Project's own `includes` directories
2. Direct dependency `src/` directories (in `[deps]` order)
3. Transitive dependency `src/` directories (depth-first)
4. Global install prefix: `~/.mx/lib/*/src/`

### Example dependency chain

```
myapp (m2.toml)
  +-- m2http (depends on m2sockets, m2tls, m2stream, m2http2, m2bytes)
       +-- m2sockets (depends on m2sys)
       +-- m2tls (depends on m2sockets; [cc] libs=-lssl -lcrypto)
       +-- m2stream (depends on m2sockets, m2tls)
       +-- m2http2 (depends on m2bytes)
       +-- m2bytes (no deps)
```

myapp's m2.toml only needs:
```toml
[deps]
m2http
```

All transitive deps (m2sockets, m2tls, m2stream, etc.) and their CC flags (`-lssl -lcrypto`, etc.) are resolved automatically.

---

## Stamp-Based Incremental Builds

Build state is stored in `.mx/build_state.json` at the project root.

For each source file involved in compilation:
- **mtime** (seconds since epoch)
- **size** (bytes)
- **hash** (FNV-1a of file contents, hex-encoded)

On subsequent `mx build`:
1. Compute stamps for all current source files
2. Compare against stored stamps
3. If all stamps match -> skip compilation entirely
4. If any stamp changed -> full recompile, update stored stamps

The `.mx/` directory is created automatically and should be added to `.gitignore`.

---

## Lockfile (m2.lock)

Generated by `mxpkg lock`. Records exact versions and hashes of resolved dependencies.

```toml
[package]
name=myapp
version=1.0.0

[dep.m2bytes]
version=0.1.0
source=installed
sha256=abc123...
path=/Users/me/.mx/lib/m2bytes
```

The compiler reads `m2.lock` if present, preferring locked paths over re-resolution.

---

## Debug Builds

```bash
mx build -g         # or mx compile src/Main.mod -g -o myapp
```

Debug mode:
- Emits `#line N "file.mod"` directives in generated C
- Two-step compile: `.c` -> `.o` -> executable (preserves debug info)
- Passes `-g -O0 -fno-omit-frame-pointer -fno-inline` to cc
- On macOS: runs `dsymutil` to generate `.dSYM` bundle

---

## Common Patterns

### Adding a library dependency

1. Install the library: `cp -r libs/m2bytes ~/.mx/lib/m2bytes` (or use mxpkg)
2. Add to m2.toml:
   ```toml
   [deps]
   m2bytes
   ```
3. Import modules in your code:
   ```modula-2
   FROM ByteBuf IMPORT Buf, Init, Free;
   ```

### Using a local library during development

```toml
[deps]
mylib=path:../mylib
```

### Linking a C library

If your library wraps a C library (e.g., SQLite):

```toml
[cc]
extra-c=src/sqlite_bridge.c
libs=-lsqlite3
```

These flags propagate transitively to any project using your library.
