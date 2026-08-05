//! `YacLib`, `EnvHandle`, `DbcHandle`, and module declarations.

mod attr;
mod conn;
mod diag;
mod raw;

use std::ffi::CStr;
use std::path::Path;

use libloading::Library;

use crate::error::Error;
use crate::load::{Loaded, MAIN_LIB_NAME};
use raw::*;

pub use conn::{DbcHandle, EnvHandle};
pub use raw::{YacCharsetCode, YacTxnIsolation};

/// Loaded yascli shared library and resolved function pointers.
pub struct YacLib {
    // The loaded library and its preloaded dependencies; kept for RAII so the
    // resolved function pointers below stay valid. `source` also records where
    // it was loaded from for the first-load-wins conflict check.
    loaded: Loaded,
    alloc_handle: YacAllocHandle,
    free_handle: YacFreeHandle,
    connect: YacConnect,
    disconnect: YacDisconnect,
    set_env_attr: YacSetEnvAttr,
    get_env_attr: YacGetEnvAttr,
    set_conn_attr: YacSetConnAttr,
    get_conn_attr: YacGetConnAttr,
    get_diag_rec: YacGetDiagRec,
}

impl YacLib {
    /// Build a `YacLib` from a loaded library, resolving all known symbols.
    pub fn from_loaded(loaded: Loaded) -> Result<Self, Error> {
        // required symbols
        let alloc_handle = try_load_symbol(&loaded.lib, c"yacAllocHandle")?;
        let free_handle = try_load_symbol(&loaded.lib, c"yacFreeHandle")?;
        let connect = try_load_symbol(&loaded.lib, c"yacConnect")?;
        let disconnect = try_load_symbol(&loaded.lib, c"yacDisconnect")?;
        let set_env_attr = try_load_symbol(&loaded.lib, c"yacSetEnvAttr")?;
        let get_env_attr = try_load_symbol(&loaded.lib, c"yacGetEnvAttr")?;
        let set_conn_attr = try_load_symbol(&loaded.lib, c"yacSetConnAttr")?;
        let get_conn_attr = try_load_symbol(&loaded.lib, c"yacGetConnAttr")?;
        let get_diag_rec = try_load_symbol(&loaded.lib, c"yacGetDiagRec")?;

        Ok(Self {
            loaded,
            alloc_handle,
            free_handle,
            connect,
            disconnect,
            set_env_attr,
            get_env_attr,
            set_conn_attr,
            get_conn_attr,
            get_diag_rec,
        })
    }

    /// True if `path` names the same library file this `YacLib` was loaded from.
    ///
    /// A library resolved via the bare-name attempt cannot be compared, so only
    /// explicit paths loaded from a known location ever match.
    pub fn source_matches(&self, path: &Path) -> bool {
        match &self.loaded.source {
            Some(source) => std::fs::canonicalize(path).map(|c| c == *source).unwrap_or(false),
            None => false,
        }
    }

    /// Human-readable description of where the library was loaded from.
    pub fn source_display(&self) -> String {
        match &self.loaded.source {
            Some(source) => source.display().to_string(),
            None => format!("{MAIN_LIB_NAME} via the default search path"),
        }
    }

    /// Call `f` and return `Err(self.get_diag_rec())` if the result is not success.
    #[inline]
    fn try_call(&self, f: impl FnOnce() -> YacResult) -> Result<(), Error> {
        match f() {
            YacResult::Success | YacResult::SuccessWithInfo => Ok(()),
            _ => {
                std::hint::cold_path();
                Err(self.get_diag_rec())
            }
        }
    }
}

#[inline]
fn try_load_symbol<T: Copy>(lib: &Library, name: &CStr) -> Result<T, Error> {
    unsafe {
        lib.get::<T>(name.to_bytes_with_nul())
            .map(|s| *s)
            .map_err(|e| Error::ClientLibrary(format!("failed to resolve symbol {}: {e}", name.to_string_lossy())))
    }
}
