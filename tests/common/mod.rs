//! Shared helpers for integration tests.
//!
//! Integration tests require the real yascli client library and a reachable
//! YashanDB instance. All connection information is supplied via environment
//! variables so tests skip cleanly (not fail) when they are not configured:
//!
//! - `YASCLI_HOME`: directory containing the yascli client library
//!   (`yascli.dll` on Windows, `libyascli.so` elsewhere).
//! - `YASDB_URL`:      database address, e.g. `172.16.91.21:16888`.
//! - `YASDB_USER`:     database username.
//! - `YASDB_PASSWORD`: database password.

// Each integration test binary compiles this module separately and only uses a
// subset of the helpers, so `dead_code` allowances are expected.
#![allow(dead_code)]

use std::path::PathBuf;

/// Environment variable holding the directory of the yascli client library.
pub const LIB_HOME_VAR: &str = "YASCLI_HOME";
/// Environment variable holding the database URL.
pub const URL_VAR: &str = "YASDB_URL";
/// Environment variable holding the database username.
pub const USER_VAR: &str = "YASDB_USER";
/// Environment variable holding the database password.
pub const PASSWORD_VAR: &str = "YASDB_PASSWORD";

/// Name of the main client library on the current platform.
pub fn main_lib_name() -> &'static str {
    if cfg!(windows) { "yascli.dll" } else { "libyascli.so" }
}

/// Path to the yascli client library, derived from `YASCLI_HOME`.
///
/// Returns `None` when `YASCLI_HOME` is unset or the library is not present.
pub fn lib_path() -> Option<PathBuf> {
    let home = std::env::var_os(LIB_HOME_VAR)?;
    let path = PathBuf::from(&home).join(main_lib_name());
    if path.exists() { Some(path) } else { None }
}

/// Connection credentials from the environment; `None` when any is unset.
pub fn conn_credentials() -> Option<(String, String, String)> {
    Some((
        std::env::var(URL_VAR).ok()?,
        std::env::var(USER_VAR).ok()?,
        std::env::var(PASSWORD_VAR).ok()?,
    ))
}

/// Load the client library from `YASCLI_HOME`; panics if it is unavailable.
///
/// Idempotent: loading the same path twice is a no-op.
pub fn require_library() {
    let path = lib_path().expect("YASCLI_HOME must name a directory containing the yascli library");
    let path_str = path.to_string_lossy().into_owned();
    yashandb::load_library_with_path(&path_str).unwrap_or_else(|e| {
        panic!("failed to load library from {path_str}: {e}");
    });
}
