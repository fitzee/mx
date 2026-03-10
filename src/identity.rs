//! Centralized compiler identity.
//!
//! All user-visible compiler branding derives from these constants.
//! Library names (m2http, m2bytes, etc.) are intentionally separate —
//! the `m2` prefix refers to the Modula-2 ecosystem, not the compiler.

/// Project / binary name.
pub const PROJECT_NAME: &str = "mx";

/// Compiler display name (used in banners, help text, diagnostics).
pub const COMPILER_NAME: &str = "mx";

/// Machine-readable compiler identifier (used in JSON output, LSP, etc.).
pub const COMPILER_ID: &str = "mx";

/// Namespace prefix for compiler-specific pragmas.
pub const PRAGMA_NAMESPACE: &str = "mx";

/// The language this compiler targets.
pub const LANGUAGE_NAME: &str = "Modula-2";

/// Dialect description.
pub const LANGUAGE_DIALECT: &str = "PIM4 with selected Plus features";

/// Compiler version (derived from Cargo.toml at compile time).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Home directory name (e.g. ~/.mx).
pub const HOME_DIR: &str = ".mx";

/// Build artifacts directory name (e.g. .mx/ inside a project).
pub const BUILD_DIR: &str = ".mx";

/// Environment variable for compiler home override.
pub const ENV_HOME: &str = "MX_HOME";

/// Environment variable to show C backend errors.
pub const ENV_SHOW_C_ERRORS: &str = "MX_SHOW_C_ERRORS";

/// Environment variable for LSP debounce.
pub const ENV_LSP_DEBOUNCE: &str = "MX_LSP_DEBOUNCE_MS";

/// Environment variable for LSP index debounce.
pub const ENV_LSP_INDEX_DEBOUNCE: &str = "MX_LSP_INDEX_DEBOUNCE_MS";

/// Environment variable for LSP tick interval.
pub const ENV_LSP_TICK: &str = "MX_LSP_TICK_MS";

/// Environment variable for docs path override.
pub const ENV_DOCS_PATH: &str = "MX_DOCS_PATH";

/// Environment variable for compiler binary override (used by mxpkg0).
pub const ENV_COMPILER: &str = "MX";
