//! Raw C type and function pointer declarations mapping `yacli.h`.
//!
//! - Type names, enum variants, and function signatures match the C header exactly.
//! - All `extern "C"` declarations, `#[repr(C)]` structs, and C enum values live here only.
//! - Enum variants drop the C prefix (e.g. `YAC_SUCCESS` → `Success`), type names keep the `Yac` prefix.
//! - Function pointer types use full parameter names for clarity.

// The raw declarations mirror the whole C surface; later iterations may not
// exercise every symbol, so dead-code allowance is scoped to this module only.
#![allow(dead_code)]

use std::ffi::c_void;

// --- C type aliases ---

pub type YacHandle = *mut c_void;
pub type YacInt16 = i16;
pub type YacInt32 = i32;

// --- C enums ---

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacResult {
    Success = 0,
    SuccessWithInfo = 1,
    Error = -1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacHandleType {
    Unknown = 0,
    Env = 1,
    Dbc = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacEnvAttr {
    CharsetCode = 62,
    SoftwareVersion = 65,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacConnAttr {
    Autocommit = 3,
    LoginTimeout = 4,
    PacketSize = 6,
    TransactionIsolation = 7,
    HeartbeatEnabled = 18,
}

// --- C structs ---

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct YacTextPos {
    pub line: YacInt32,
    pub column: YacInt32,
}

// --- Function pointer types ---

pub type YacAllocHandle =
    unsafe extern "C" fn(handle_type: YacHandleType, input: YacHandle, output: *mut YacHandle) -> YacResult;

pub type YacFreeHandle = unsafe extern "C" fn(handle_type: YacHandleType, handle: YacHandle) -> YacResult;

pub type YacConnect = unsafe extern "C" fn(
    hconn: YacHandle,
    url: *const u8,
    url_len: YacInt16,
    username: *const u8,
    username_len: YacInt16,
    password: *const u8,
    password_len: YacInt16,
) -> YacResult;

pub type YacDisconnect = unsafe extern "C" fn(hconn: YacHandle);

pub type YacSetEnvAttr =
    unsafe extern "C" fn(henv: YacHandle, attr: YacEnvAttr, value: *mut c_void, length: YacInt32) -> YacResult;

pub type YacGetEnvAttr = unsafe extern "C" fn(
    henv: YacHandle,
    attr: YacEnvAttr,
    value: *mut c_void,
    buf_len: YacInt32,
    string_length: *mut YacInt32,
) -> YacResult;

pub type YacSetConnAttr =
    unsafe extern "C" fn(hconn: YacHandle, attr: YacConnAttr, value: *mut c_void, length: YacInt32) -> YacResult;

pub type YacGetConnAttr = unsafe extern "C" fn(
    hconn: YacHandle,
    attr: YacConnAttr,
    value: *mut c_void,
    buf_len: YacInt32,
    string_length: *mut YacInt32,
) -> YacResult;

pub type YacGetDiagRec = unsafe extern "C" fn(
    err_code: *mut YacInt32,
    message: *mut u8,
    buf_len: YacInt32,
    indicator: *mut YacInt32,
    sql_state: *mut u8,
    sql_state_len: YacInt32,
    pos: *mut YacTextPos,
) -> YacResult;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_repr() {
        assert_eq!(YacResult::Success as i32, 0);
        assert_eq!(YacResult::SuccessWithInfo as i32, 1);
        assert_eq!(YacResult::Error as i32, -1);
    }

    #[test]
    fn handle_type_repr() {
        assert_eq!(YacHandleType::Unknown as i32, 0);
        assert_eq!(YacHandleType::Env as i32, 1);
        assert_eq!(YacHandleType::Dbc as i32, 2);
    }

    #[test]
    fn env_attr_repr() {
        assert_eq!(YacEnvAttr::CharsetCode as i32, 62);
        assert_eq!(YacEnvAttr::SoftwareVersion as i32, 65);
    }

    #[test]
    fn conn_attr_repr() {
        assert_eq!(YacConnAttr::Autocommit as i32, 3);
        assert_eq!(YacConnAttr::LoginTimeout as i32, 4);
        assert_eq!(YacConnAttr::PacketSize as i32, 6);
        assert_eq!(YacConnAttr::TransactionIsolation as i32, 7);
        assert_eq!(YacConnAttr::HeartbeatEnabled as i32, 18);
    }
}
