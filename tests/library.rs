//! Integration tests for client library loading.
//!
//! All tests share one process and therefore one global `YAC_LIB` singleton, so
//! they must run serially and respect the first-load-wins rule. Tests are
//! skipped (not failed) when `YASCLI_HOME` is not configured.

mod common;

use std::sync::Mutex;

use common::{LIB_HOME_VAR, lib_path, main_lib_name};

/// Serializes tests that touch the process-global library state.
static LIB_LOCK: Mutex<()> = Mutex::new(());

/// Load the library if it is not already loaded from a different path.
fn ensure_loaded() -> Option<String> {
    let path = lib_path()?;
    let path_str = path.to_string_lossy().into_owned();
    yashandb::load_library_with_path(&path_str).expect("load library from YASCLI_HOME");
    Some(path_str)
}

#[test]
fn load_from_explicit_path_ok() {
    let _g = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = ensure_loaded() else {
        eprintln!("skipping: {LIB_HOME_VAR} not set or library missing");
        return;
    };
    // Second load of the same path is idempotent (no conflict).
    yashandb::load_library_with_path(&path).unwrap();
}

#[test]
fn load_auto_discovery_ok() {
    let _g = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(_) = ensure_loaded() else {
        eprintln!("skipping: {LIB_HOME_VAR} not set or library missing");
        return;
    };
    // Auto-discovery never conflicts with an already-loaded library.
    yashandb::load_library().unwrap();
}

#[test]
fn load_conflicting_path_errors() {
    let _g = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(_) = ensure_loaded() else {
        eprintln!("skipping: {LIB_HOME_VAR} not set or library missing");
        return;
    };
    // A non-existent path under the temp dir, so it cannot be the same file as
    // the loaded library (canonicalize fails, `source_matches` returns false).
    let other = std::env::temp_dir().join(format!("other-{}-{}", main_lib_name(), std::process::id()));
    let err = yashandb::load_library_with_path(other.to_str().unwrap()).unwrap_err();
    match &err {
        yashandb::Error::ClientLibrary(msg) => {
            assert!(msg.contains("refusing to load from"), "unexpected message: {msg}");
            // The load source is reported (canonicalized; on Windows this may
            // carry a `\\?\` prefix, so only check the library name).
            assert!(msg.contains(main_lib_name()), "unexpected message: {msg}");
        }
        other => panic!("expected ClientLibrary, got {other:?}"),
    }
}

#[test]
fn load_missing_path_errors() {
    let _g = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = ensure_loaded();
    let missing = std::env::temp_dir()
        .join("no")
        .join("such")
        .join("dir")
        .join(main_lib_name());
    let err = yashandb::load_library_with_path(missing.to_str().unwrap()).unwrap_err();
    // Whether the library is already loaded changes the exact message (load
    // failure vs. first-load-wins conflict), but both are ClientLibrary.
    assert!(matches!(err, yashandb::Error::ClientLibrary(_)));
}
