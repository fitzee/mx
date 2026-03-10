# Versioning Policy

## Compiler (mx)

The compiler follows [semver](https://semver.org/): **MAJOR.MINOR.PATCH**.

- **MAJOR** — breaking changes to language semantics, C codegen ABI, or CLI interface
- **MINOR** — new language features, new builtins, new CLI flags, new bundled library
- **PATCH** — bug fixes, codegen correctness, test coverage improvements

**Source of truth:** `Cargo.toml` version field, read at build time via `env!("CARGO_PKG_VERSION")` and surfaced through `src/identity.rs`.

## Libraries (m2*)

Each library has its own independent semver version in its `m2.toml` manifest (`version=` field).

- **0.x libraries** — any minor bump may include breaking API changes (per semver spec)
- **1.0.0+ libraries** — standard semver guarantees apply; breaking changes require a major bump

### Graduation criteria for 1.0.0

A library graduates to 1.0.0 when all three conditions are met:

1. **Stable public API** — no `.def` changes in 2+ compiler releases
2. **Downstream consumer** — used by at least one other library, tool, or example app
3. **No planned rework** — no known API design issues flagged for redesign

## Tools (mxpkg, mxpkg0, VS Code extension)

- **mxpkg** — version in `tools/mxpkg/m2.toml`
- **mxpkg0** — version in `tools/mxpkg0/Cargo.toml`
- **VS Code extension** — version in `tools/vscode-m2plus/package.json`

All tools follow semver independently. CLI flags and subcommands are part of the API surface.

## Compiler–library coupling

Libraries ship with the compiler but are **not** version-locked to it.

- A compiler release bundles specific library versions, recorded in a [release manifest](#release-manifests)
- Upgrading the compiler may upgrade bundled libraries; the manifest documents exactly which versions
- Libraries can be updated independently via mxpkg when the registry is available

## Schema/format versions

Internal format versions (`manifest_version`, `plan_version`, etc.) are integer counters, not semver. Increment only on breaking format changes.

## Release manifests

Each compiler release has a corresponding `releases/mx-X.Y.Z.toml` file recording all bundled component versions. This is the machine-readable record of what shipped together.

## Release process

1. Update `Cargo.toml` version
2. Update `README.md` "What's new" section
3. Create `releases/mx-X.Y.Z.toml` with all bundled versions
4. Tag the git commit with `vX.Y.Z`
