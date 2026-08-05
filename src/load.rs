//! Dynamic library discovery and loading.
//!
//! Replicates the loading strategy of the Go driver (`yacapi/src/yapi_cli.c`,
//! `yapiOpenDynamicLib`):
//!
//! 1. Try the bare library name first (`libyascli.so` / `yascli.dll`), letting
//!    the platform's default search path resolve it (`LD_LIBRARY_PATH`,
//!    `ld.so.cache`, PATH, application directory, ...).
//! 2. If that fails, fall back to the per-user install location
//!    `$HOME/.yashandb/client/lib` (Linux/macOS) or
//!    `%USERPROFILE%\.yashandb\client\lib` (Windows), preloading the bundled
//!    dependency libraries first so the main library's dependencies resolve.
//!
//! The per-user directory is *not* on either platform's default search path, so
//! dependencies must be registered in the process's loaded-library table by
//! `dlopen`-ing them with their absolute path before the main library is loaded.
//! Preloading is best-effort, matching the Go driver: a failed preload only
//! surfaces as a failure to load the main library.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use libloading::Library;

use crate::error::Error;

/// Name of the main client library.
#[cfg(windows)]
pub const MAIN_LIB_NAME: &str = "yascli.dll";
#[cfg(not(windows))]
pub const MAIN_LIB_NAME: &str = "libyascli.so";

/// Dependency libraries shipped next to the main library. The Go driver preloads
/// these before loading the main library from the per-user directory so that its
/// dependency resolution finds the bundled copies instead of failing.
#[cfg(windows)]
const DEP_LIB_NAMES: &[&str] = &["libssl-1_1-x64.dll", "libcrypto-1_1-x64.dll", "yas_infra.dll"];
#[cfg(not(windows))]
const DEP_LIB_NAMES: &[&str] = &["libcrypto.so", "libssl.so", "libyas_infra.so"];

/// A successfully loaded main library plus any dependency libraries preloaded
/// to satisfy its dependencies. All handles must stay alive together.
pub struct Loaded {
    /// The main library. Declared first so it is dropped first, while the
    /// dependency handles below are still alive.
    pub lib: Library,
    /// Preloaded dependency handles; kept for RAII, never read.
    #[allow(dead_code)]
    pub deps: Vec<Library>,
    /// Canonical path the main library was loaded from, when known (`None` when
    /// resolved via the bare-name/default-search attempt).
    pub source: Option<PathBuf>,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Library` doesn't implement `Debug`; print the useful parts only.
        f.debug_struct("Loaded")
            .field("source", &self.source)
            .field("deps", &self.deps.len())
            .finish()
    }
}

/// Load the main library: bare name first, then the per-user directory with
/// dependency preloading.
pub fn load_auto() -> Result<Loaded, Error> {
    let mut tried = Vec::new();
    let mut deps = Vec::new();

    // 1. Bare name: resolved via the platform's default search path.
    let bare = unsafe { Library::new(OsStr::new(MAIN_LIB_NAME)) };
    if let Ok(lib) = bare {
        return Ok(Loaded {
            lib,
            deps: Vec::new(),
            source: None,
        });
    }
    tried.push(format!("{MAIN_LIB_NAME}: {}", bare.unwrap_err()));

    // 2. Per-user directory, preloading the bundled dependencies first.
    let Some(dir) = user_lib_dir() else {
        return Err(not_found(&tried));
    };
    preload_deps_from(&dir, &mut deps);
    let main = dir.join(MAIN_LIB_NAME);
    match unsafe { Library::new(main.as_os_str()) } {
        Ok(lib) => {
            let source = std::fs::canonicalize(&main).unwrap_or(main);
            Ok(Loaded {
                lib,
                deps,
                source: Some(source),
            })
        }
        Err(e) => {
            tried.push(format!("{}: {e}", main.display()));
            Err(not_found(&tried))
        }
    }
}

/// Load the main library from an explicit path, preloading the standard
/// dependency libraries from the same directory (best-effort, mirroring the
/// per-user fallback above).
pub fn load_explicit(path: &Path) -> Result<Loaded, Error> {
    let mut deps = Vec::new();
    if let Some(dir) = path.parent() {
        preload_deps_from(dir, &mut deps);
    }
    let lib = unsafe { Library::new(path.as_os_str()) }
        .map_err(|e| Error::ClientLibrary(format!("failed to load {}: {e}", path.display())))?;
    let source = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(Loaded {
        lib,
        deps,
        source: Some(source),
    })
}

/// The per-user YashanDB client library directory.
fn user_lib_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE")?;
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME")?;
    Some(user_lib_dir_from(&PathBuf::from(home)))
}

/// Build the per-user client library directory from a home directory.
fn user_lib_dir_from(home: &Path) -> PathBuf {
    home.join(".yashandb").join("client").join("lib")
}

/// Best-effort `dlopen` of each bundled dependency from `dir`. Errors are
/// ignored: if the main library turns out not to need them, nothing is lost.
fn preload_deps_from(dir: &Path, out: &mut Vec<Library>) {
    for dep in DEP_LIB_NAMES {
        if let Ok(h) = unsafe { Library::new(dir.join(dep).as_os_str()) } {
            out.push(h);
        }
    }
}

fn not_found(tried: &[String]) -> Error {
    Error::ClientLibrary(format!("failed to load {MAIN_LIB_NAME}; tried {}", tried.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Component;

    #[test]
    fn user_dir_segments() {
        use std::ffi::OsStr;
        let home = Path::new("usr").join("alice");
        let dir = user_lib_dir_from(&home);
        let mut parts = dir.components();
        assert_eq!(parts.next(), Some(Component::Normal(OsStr::new("usr"))));
        assert_eq!(parts.next(), Some(Component::Normal(OsStr::new("alice"))));
        assert_eq!(parts.next(), Some(Component::Normal(OsStr::new(".yashandb"))));
        assert_eq!(parts.next(), Some(Component::Normal(OsStr::new("client"))));
        assert_eq!(parts.next(), Some(Component::Normal(OsStr::new("lib"))));
        assert_eq!(parts.next(), None);
    }

    #[test]
    fn user_dir_from_env_is_absolute_prefixed() {
        // The constructed dir must stay under the home directory.
        assert!(user_lib_dir_from(Path::new("h")).starts_with("h"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_lib_names() {
        assert_eq!(MAIN_LIB_NAME, "yascli.dll");
        assert_eq!(
            DEP_LIB_NAMES,
            ["libssl-1_1-x64.dll", "libcrypto-1_1-x64.dll", "yas_infra.dll"]
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_lib_names() {
        assert_eq!(MAIN_LIB_NAME, "libyascli.so");
        assert_eq!(DEP_LIB_NAMES, ["libcrypto.so", "libssl.so", "libyas_infra.so"]);
    }

    // --- load_explicit error paths (no external library needed) ---

    #[test]
    fn load_explicit_missing_path_errors() {
        let path = Path::new("no").join("such").join("dir").join("yascli.lib");
        let err = load_explicit(&path).unwrap_err();
        match &err {
            Error::ClientLibrary(msg) => {
                assert!(msg.contains("failed to load"), "unexpected message: {msg}");
                assert!(msg.contains("yascli.lib"), "unexpected message: {msg}");
            }
            other => panic!("expected ClientLibrary, got {other:?}"),
        }
    }

    #[test]
    fn load_explicit_non_library_file_errors() {
        // Cargo.toml exists but is not a dynamic library.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let err = load_explicit(&path).unwrap_err();
        assert!(matches!(err, Error::ClientLibrary(_)));
    }

    #[test]
    fn load_explicit_directory_errors() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let err = load_explicit(path).unwrap_err();
        assert!(matches!(err, Error::ClientLibrary(_)));
    }

    #[test]
    fn not_found_message_lists_tried_sources() {
        let tried = vec!["first: reason".to_string(), "second: boom".to_string()];
        let err = not_found(&tried);
        let Error::ClientLibrary(msg) = err else {
            panic!("expected ClientLibrary, got {err:?}");
        };
        assert!(msg.contains(MAIN_LIB_NAME), "unexpected message: {msg}");
        assert!(msg.contains("first: reason"), "unexpected message: {msg}");
        assert!(msg.contains("second: boom"), "unexpected message: {msg}");
    }

    // --- real-library tests, gated on YASCLI_HOME ---

    /// Load the real `yascli` library from `$YASCLI_HOME` via an explicit path.
    /// Skipped (not failed) when `YASCLI_HOME` is unset.
    #[test]
    fn load_explicit_with_real_library() {
        let Some(home) = std::env::var_os("YASCLI_HOME") else {
            eprintln!("skipping: YASCLI_HOME not set");
            return;
        };
        let path = Path::new(&home).join(MAIN_LIB_NAME);
        let loaded = load_explicit(&path).unwrap_or_else(|e| panic!("failed to load from {}: {e}", path.display()));
        // An explicit load must record the canonical path it was loaded from.
        let source = loaded.source.expect("explicit load records a source");
        // The recorded source must be an absolute path naming the same file that
        // was loaded (canonicalize also verifies the file exists).
        assert!(source.is_absolute(), "source not absolute: {source:?}");
        assert_eq!(
            std::fs::canonicalize(&source).unwrap(),
            std::fs::canonicalize(&path).unwrap()
        );
    }
}
