//! Global loaded-library state and loading entry points.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::error::Error;
use crate::ffi::YacLib;
use crate::load;

static YAC_LIB: OnceLock<YacLib> = OnceLock::new();

/// Return the loaded library, loading it on first use.
///
/// `custom` overrides auto-discovery for the initial load. If the library is
/// already loaded, a `custom` path is rejected when it does not name the same
/// file (`first load wins`); auto-discovery never conflicts. A failed load
/// leaves the library unloaded, so a later call retries.
pub(crate) fn library(custom: Option<&Path>) -> Result<&'static YacLib, Error> {
    // Serializes the initial load so concurrent triggers attempt it exactly
    // once. Function-local so no other code can reach it.
    // (`OnceLock::get_or_try_init` would do this natively but is not stable yet.)
    static INIT_LOCK: Mutex<()> = Mutex::new(());

    if let Some(lib) = YAC_LIB.get() {
        return check_source(custom, lib);
    }

    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(lib) = YAC_LIB.get() {
        return check_source(custom, lib);
    }

    let loaded = match custom {
        Some(path) => load::load_explicit(path)?,
        None => load::load_auto()?,
    };
    let lib = YacLib::from_loaded(loaded)?;
    // Cannot fail: the lock is held and the re-check above ensures nothing was
    // set in between. (`.ok()` avoids a `Debug` bound on `YacLib`.)
    YAC_LIB.set(lib).ok().expect("YAC_LIB already initialized");
    Ok(YAC_LIB.get().expect("just initialized"))
}

/// Enforce the first-load-wins rule for an explicit `custom` path.
fn check_source<'a>(custom: Option<&Path>, lib: &'a YacLib) -> Result<&'a YacLib, Error> {
    if let Some(path) = custom
        && !lib.source_matches(path)
    {
        return Err(Error::ClientLibrary(format!(
            "library already loaded from {}, refusing to load from {}",
            lib.source_display(),
            path.display()
        )));
    }
    Ok(lib)
}

/// Load the yascli shared library via auto-discovery: the default search path
/// first, then `$HOME/.yashandb/client/lib` (Linux/macOS) or
/// `%USERPROFILE%\.yashandb\client\lib` (Windows) with the bundled dependency
/// libraries preloaded.
///
/// The library is also loaded automatically by the first `connect`; this is
/// only needed to fail fast at startup. Idempotent: `Ok` if already loaded.
#[inline]
pub fn load_library() -> Result<(), Error> {
    library(None).map(|_| ())
}

/// Load the yascli shared library from an explicit path.
///
/// Idempotent for the same file; `Err` if the library was already loaded from a
/// different source. On failure the library is left unloaded, so a later
/// `load_library()` or `connect()` retries with auto-discovery.
#[inline]
pub fn load_library_with_path(path: &str) -> Result<(), Error> {
    library(Some(Path::new(path))).map(|_| ())
}

/// The loaded library; `connect` has already loaded it by construction.
#[inline]
pub(crate) fn loaded_library() -> &'static YacLib {
    YAC_LIB.get().expect("connection implies library loaded")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::MAIN_LIB_NAME;

    // `YAC_LIB` is a process-global `OnceLock` that cannot be reset, so the
    // library-level tests share one process and must run serially. They are
    // gated on `YASCLI_HOME` and skipped (not failed) when it is unset.
    static LIB_LOCK: Mutex<()> = Mutex::new(());

    fn real_lib_path() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("YASCLI_HOME")?;
        Some(Path::new(&home).join(MAIN_LIB_NAME))
    }

    #[test]
    #[cfg_attr(all(miri, windows), ignore = "SetThreadErrorMode is unsupported under Miri on Windows")]
    #[cfg_attr(all(miri, not(windows)), ignore = "dlopen is unsupported under Miri")]
    fn load_with_path_then_idempotent_then_first_load_wins() {
        let _g = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = real_lib_path() else {
            eprintln!("skipping: YASCLI_HOME not set");
            return;
        };
        // A path guaranteed to differ from the loaded library's: a file in the
        // system temp dir would not exist as a sibling of the real library, so
        // `source_matches` cannot be fooled by a stray hardlink/symlink.
        let other = std::env::temp_dir().join(format!("other-{MAIN_LIB_NAME}-{}", std::process::id()));

        // First explicit load succeeds.
        load_library_with_path(path.to_str().unwrap()).unwrap();

        // Idempotent: the same file again is Ok.
        load_library_with_path(path.to_str().unwrap()).unwrap();

        // Auto-discovery on an already-loaded library never conflicts.
        load_library().unwrap();

        // First-load-wins: a different path is rejected with the specific
        // conflict message (not a generic load failure).
        let err = load_library_with_path(other.to_str().unwrap()).unwrap_err();
        let Error::ClientLibrary(msg) = &err else {
            panic!("expected ClientLibrary, got {err:?}");
        };
        assert!(msg.contains("refusing to load from"), "unexpected message: {msg}");
    }
}
