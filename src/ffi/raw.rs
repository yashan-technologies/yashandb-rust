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
pub type YacInt8 = i8;
pub type YacInt16 = i16;
pub type YacInt32 = i32;
pub type YacInt64 = i64;
pub type YacUint8 = u8;
pub type YacUint16 = u16;
pub type YacUint32 = u32;
pub type YacUint64 = u64;
pub type YacDate = i64;
pub type YacShortTime = i64;
pub type YacYMInterval = i32;
pub type YacDSInterval = i64;

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
    Stmt = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum YacType {
    Unknown = 0,
    Bool = 1,
    TinyInt = 2,
    SmallInt = 3,
    Integer = 4,
    BigInt = 5,
    UTinyInt = 6,
    USmallInt = 7,
    UInteger = 8,
    UBigInt = 9,
    Float = 10,
    Double = 11,
    Number = 12,
    Date = 13,
    ShortTime = 15,
    Timestamp = 16,
    TimestampLtz = 17,
    TimestampTz = 18,
    YmInterval = 19,
    DsInterval = 20,
    Char = 24,
    NChar = 25,
    VarChar = 26,
    NVarChar = 27,
    Binary = 28,
    Clob = 29,
    Blob = 30,
    Bit = 31,
    RowId = 32,
    NClob = 33,
    Cursor = 34,
    Json = 35,
    Xml = 39,
    NumericFloat = 40,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacParamDirection {
    Input = 1,
    Output = 2,
    InOut = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum YacExtType {
    Unknown = 0,
    Bool = 1,
    TinyInt = 2,
    SmallInt = 3,
    Integer = 4,
    BigInt = 5,
    Float = 10,
    Double = 11,
    Number = 12,
    Date = 13,
    ShortTime = 15,
    Timestamp = 16,
    YmInterval = 19,
    DsInterval = 20,
    Char = 24,
    VarChar = 26,
    Binary = 28,
    Char2 = 100,
    Varchar2 = 101,
    Binary2 = 102,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacStmtAttr {
    RowsetSize = 101,
    RowsAffected = 103,
    CursorEof = 104,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacColAttr {
    Name = 1,
    Size = 2,
    Type = 3,
    Precision = 4,
    Scale = 5,
    Nullable = 6,
    CharSize = 7,
}

pub const YAC_NULL_DATA: YacInt32 = -1;

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub struct YacNumber {
    pub number_part: [u8; 20],
}

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub struct YacTimestamp {
    pub timestamp_part: [u8; 12],
}

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub struct YacRowId {
    pub row_id_part: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacEnvAttr {
    CharsetCode = 62,
    ReturnSuccessWithInfo = 64,
    SoftwareVersion = 65,
    ClientDriver = 66,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacConnAttr {
    AutoCommit = 3,
    LoginTimeout = 4,
    PacketSize = 6,
    TransactionIsolation = 7,
    Credt = 11,
    MaxCharsetRatio = 12,
    TafEnabled = 14,
    TafCallback = 15,
    MaxNcharsetRatio = 17,
    HeartbeatEnabled = 18,
}

/// Client character set code (`YacCharsetCode`).
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacCharsetCode {
    ASCII = 0,
    GBK = 1,
    UTF8 = 2,
    ISO88591 = 3,
    UTF16 = 4,
    GB18030 = 5,
}

/// Connection transaction isolation level (`YacTxnIsolation`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum YacTxnIsolation {
    ReadCommitted = 0,
    CurrCommitted = 1,
    Serializable = 2,
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
    h_conn: YacHandle,
    url: *const u8,
    url_len: YacInt16,
    user: *const u8,
    user_len: YacInt16,
    pwd: *const u8,
    pwd_len: YacInt16,
) -> YacResult;

pub type YacDisconnect = unsafe extern "C" fn(h_conn: YacHandle);

pub type YacSetEnvAttr =
    unsafe extern "C" fn(h_env: YacHandle, attr: YacEnvAttr, value: *mut c_void, length: YacInt32) -> YacResult;

pub type YacGetEnvAttr = unsafe extern "C" fn(
    h_env: YacHandle,
    attr: YacEnvAttr,
    value: *mut c_void,
    buf_len: YacInt32,
    string_length: *mut YacInt32,
) -> YacResult;

pub type YacSetConnAttr =
    unsafe extern "C" fn(h_conn: YacHandle, attr: YacConnAttr, value: *mut c_void, length: YacInt32) -> YacResult;

pub type YacGetConnAttr = unsafe extern "C" fn(
    h_conn: YacHandle,
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
    sql_state_buf_len: YacInt32,
    pos: *mut YacTextPos,
) -> YacResult;

pub type YacDirectExecute = unsafe extern "C" fn(h_stmt: YacHandle, sql: *const u8, sql_length: YacInt32) -> YacResult;

pub type YacPrepare = unsafe extern "C" fn(h_stmt: YacHandle, sql: *const u8, sql_length: YacInt32) -> YacResult;

pub type YacExecute = unsafe extern "C" fn(h_stmt: YacHandle) -> YacResult;

pub type YacBindParameter = unsafe extern "C" fn(
    h_stmt: YacHandle,
    id: YacUint16,
    direction: YacParamDirection,
    ext_type: YacUint32,
    value: *mut c_void,
    bind_size: YacInt32,
    buf_length: YacInt32,
    indicator: *mut YacInt32,
) -> YacResult;

pub type YacBindParameterByName = unsafe extern "C" fn(
    h_stmt: YacHandle,
    name: *mut u8,
    direction: YacParamDirection,
    ext_type: YacUint32,
    value: *mut c_void,
    bind_size: YacInt32,
    buf_length: YacInt32,
    indicator: *mut YacInt32,
) -> YacResult;

pub type YacNumParams = unsafe extern "C" fn(h_stmt: YacHandle, count: *mut YacUint16) -> YacResult;

pub type YacFetch = unsafe extern "C" fn(h_stmt: YacHandle, rows: *mut YacUint32) -> YacResult;

pub type YacSetStmtAttr =
    unsafe extern "C" fn(h_stmt: YacHandle, attr: YacStmtAttr, value: *mut c_void, length: YacInt32) -> YacResult;

pub type YacGetStmtAttr = unsafe extern "C" fn(
    h_stmt: YacHandle,
    attr: YacStmtAttr,
    value: *mut c_void,
    buf_len: YacInt32,
    string_length: *mut YacInt32,
) -> YacResult;

pub type YacBindColumn = unsafe extern "C" fn(
    h_stmt: YacHandle,
    id: YacUint16,
    ext_type: YacUint32,
    value: *mut c_void,
    buf_len: YacInt32,
    indicator: *mut YacInt32,
) -> YacResult;

pub type YacNumResultCols = unsafe extern "C" fn(h_stmt: YacHandle, count: *mut YacInt16) -> YacResult;

pub type YacColAttribute = unsafe extern "C" fn(
    h_stmt: YacHandle,
    id: YacUint16,
    attr: YacColAttr,
    value: *mut c_void,
    buf_len: YacInt32,
    string_length: *mut YacInt32,
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
    fn param_direction_repr() {
        assert_eq!(YacParamDirection::Input as i32, 1);
        assert_eq!(YacParamDirection::Output as i32, 2);
        assert_eq!(YacParamDirection::InOut as i32, 3);
    }

    #[test]
    fn env_attr_repr() {
        assert_eq!(YacEnvAttr::CharsetCode as i32, 62);
        assert_eq!(YacEnvAttr::ReturnSuccessWithInfo as i32, 64);
        assert_eq!(YacEnvAttr::SoftwareVersion as i32, 65);
        assert_eq!(YacEnvAttr::ClientDriver as i32, 66);
    }

    #[test]
    fn conn_attr_repr() {
        assert_eq!(YacConnAttr::AutoCommit as i32, 3);
        assert_eq!(YacConnAttr::LoginTimeout as i32, 4);
        assert_eq!(YacConnAttr::PacketSize as i32, 6);
        assert_eq!(YacConnAttr::TransactionIsolation as i32, 7);
        assert_eq!(YacConnAttr::Credt as i32, 11);
        assert_eq!(YacConnAttr::MaxCharsetRatio as i32, 12);
        assert_eq!(YacConnAttr::TafEnabled as i32, 14);
        assert_eq!(YacConnAttr::TafCallback as i32, 15);
        assert_eq!(YacConnAttr::MaxNcharsetRatio as i32, 17);
        assert_eq!(YacConnAttr::HeartbeatEnabled as i32, 18);
    }

    #[test]
    fn charset_code_repr() {
        assert_eq!(YacCharsetCode::ASCII as i32, 0);
        assert_eq!(YacCharsetCode::GBK as i32, 1);
        assert_eq!(YacCharsetCode::UTF8 as i32, 2);
        assert_eq!(YacCharsetCode::ISO88591 as i32, 3);
        assert_eq!(YacCharsetCode::UTF16 as i32, 4);
        assert_eq!(YacCharsetCode::GB18030 as i32, 5);
    }

    #[test]
    fn txn_isolation_repr() {
        assert_eq!(YacTxnIsolation::ReadCommitted as i32, 0);
        assert_eq!(YacTxnIsolation::CurrCommitted as i32, 1);
        assert_eq!(YacTxnIsolation::Serializable as i32, 2);
    }
}
