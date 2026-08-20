//! `YacLib`, `EnvHandle`, `DbcHandle`, and module declarations.

mod attr;
mod conn;
mod diag;
mod raw;
mod stmt;

use std::ffi::CStr;
use std::path::Path;

use libloading::Library;

use crate::error::Error;
use crate::load::{Loaded, MAIN_LIB_NAME};
use raw::*;

pub use conn::{DbcHandle, EnvHandle};
pub use raw::{YacCharsetCode, YacExtType, YacTxnIsolation, YacType};
pub use stmt::{ParameterBinding, ParameterValue, StmtHandle};

pub const NULL_DATA: i32 = -1;

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
    commit: YacCommit,
    rollback: YacRollback,
    set_env_attr: YacSetEnvAttr,
    get_env_attr: YacGetEnvAttr,
    set_conn_attr: YacSetConnAttr,
    get_conn_attr: YacGetConnAttr,
    get_diag_rec: YacGetDiagRec,
    direct_execute: YacDirectExecute,
    prepare: YacPrepare,
    execute: YacExecute,
    bind_parameter: YacBindParameter,
    bind_parameter_by_name: YacBindParameterByName,
    num_params: YacNumParams,
    fetch: YacFetch,
    get_stmt_attr: YacGetStmtAttr,
    bind_column: YacBindColumn,
    num_result_cols: YacNumResultCols,
    col_attribute: YacColAttribute,
}

impl YacLib {
    /// Build a `YacLib` from a loaded library, resolving all known symbols.
    pub fn from_loaded(loaded: Loaded) -> Result<Self, Error> {
        // required symbols
        let alloc_handle = try_load_symbol(&loaded.lib, c"yacAllocHandle")?;
        let free_handle = try_load_symbol(&loaded.lib, c"yacFreeHandle")?;
        let connect = try_load_symbol(&loaded.lib, c"yacConnect")?;
        let disconnect = try_load_symbol(&loaded.lib, c"yacDisconnect")?;
        let commit = try_load_symbol(&loaded.lib, c"yacCommit")?;
        let rollback = try_load_symbol(&loaded.lib, c"yacRollback")?;
        let set_env_attr = try_load_symbol(&loaded.lib, c"yacSetEnvAttr")?;
        let get_env_attr = try_load_symbol(&loaded.lib, c"yacGetEnvAttr")?;
        let set_conn_attr = try_load_symbol(&loaded.lib, c"yacSetConnAttr")?;
        let get_conn_attr = try_load_symbol(&loaded.lib, c"yacGetConnAttr")?;
        let get_diag_rec = try_load_symbol(&loaded.lib, c"yacGetDiagRec")?;
        let direct_execute = try_load_symbol(&loaded.lib, c"yacDirectExecute")?;
        let prepare = try_load_symbol(&loaded.lib, c"yacPrepare")?;
        let execute = try_load_symbol(&loaded.lib, c"yacExecute")?;
        let bind_parameter = try_load_symbol(&loaded.lib, c"yacBindParameter")?;
        let bind_parameter_by_name = try_load_symbol(&loaded.lib, c"yacBindParameterByName")?;
        let num_params = try_load_symbol(&loaded.lib, c"yacNumParams")?;
        let fetch = try_load_symbol(&loaded.lib, c"yacFetch")?;
        let get_stmt_attr = try_load_symbol(&loaded.lib, c"yacGetStmtAttr")?;
        let bind_column = try_load_symbol(&loaded.lib, c"yacBindColumn")?;
        let num_result_cols = try_load_symbol(&loaded.lib, c"yacNumResultCols")?;
        let col_attribute = try_load_symbol(&loaded.lib, c"yacColAttribute")?;

        Ok(Self {
            loaded,
            alloc_handle,
            free_handle,
            connect,
            disconnect,
            commit,
            rollback,
            set_env_attr,
            get_env_attr,
            set_conn_attr,
            get_conn_attr,
            get_diag_rec,
            direct_execute,
            prepare,
            execute,
            bind_parameter,
            bind_parameter_by_name,
            num_params,
            fetch,
            get_stmt_attr,
            bind_column,
            num_result_cols,
            col_attribute,
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
