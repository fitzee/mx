//! LSP workspace support — re-exports project resolver types.
//!
//! All manifest/lockfile parsing lives in `crate::project_resolver`.
//! This module re-exports the types for backward compatibility with
//! existing LSP code.

pub use crate::project_resolver::{
    Manifest, DepEntry, DepSource, CcSection, TestSection,
    Lockfile, LockDep,
    ProjectContext,
    find_project_root, resolve_include_paths,
};
