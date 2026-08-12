//! Env/dbc attribute get/set via `yacSetEnvAttr`, `yacGetEnvAttr`, `yacSetConnAttr`, `yacGetConnAttr`.

use std::ffi::c_void;

use crate::error::Error;
use crate::ffi::raw::*;
use crate::ffi::{DbcHandle, EnvHandle, YacLib};

impl YacLib {
    // Handles below take `&mut` even when only read, matching the conn.rs
    // convention: the `&mut` borrow guarantees exclusive access so the same C
    // handle is never touched concurrently from multiple threads.

    // --- 4 basic functions ---

    /// Set an env attribute. `value` points at the raw bytes; `length` is the byte count.
    #[inline]
    fn set_env_attr(
        &self,
        env: &mut EnvHandle,
        attr: YacEnvAttr,
        value: *mut c_void,
        length: YacInt32,
    ) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.set_env_attr)(env.as_ptr(), attr, value, length) })
    }

    /// Get an env attribute into a caller-provided buffer of `buf_len` bytes.
    ///
    /// On success the actual byte length is written to `string_length`.
    #[allow(dead_code)]
    #[inline]
    fn get_env_attr(
        &self,
        env: &mut EnvHandle,
        attr: YacEnvAttr,
        value: *mut c_void,
        buf_len: YacInt32,
        string_length: &mut YacInt32,
    ) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.get_env_attr)(env.as_ptr(), attr, value, buf_len, string_length) })
    }

    /// Set a connection attribute. `value` points at the raw bytes; `length` is the byte count.
    #[inline]
    fn set_conn_attr(
        &self,
        dbc: &mut DbcHandle,
        attr: YacConnAttr,
        value: *mut c_void,
        length: YacInt32,
    ) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.set_conn_attr)(dbc.as_ptr(), attr, value, length) })
    }

    /// Get a connection attribute into a caller-provided buffer of `buf_len` bytes.
    ///
    /// On success the actual byte length is written to `string_length`.
    #[inline]
    fn get_conn_attr(
        &self,
        dbc: &mut DbcHandle,
        attr: YacConnAttr,
        value: *mut c_void,
        buf_len: YacInt32,
        string_length: &mut YacInt32,
    ) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.get_conn_attr)(dbc.as_ptr(), attr, value, buf_len, string_length) })
    }

    // --- typed helpers ---

    /// Set a connection attribute from an `i32` value.
    #[inline]
    fn set_conn_attr_i32(&self, dbc: &mut DbcHandle, attr: YacConnAttr, v: i32) -> Result<(), Error> {
        self.set_conn_attr(
            dbc,
            attr,
            &v as *const i32 as *mut c_void,
            std::mem::size_of::<i32>() as i32,
        )
    }

    /// Set a connection attribute from a `u32` value.
    #[inline]
    fn set_conn_attr_u32(&self, dbc: &mut DbcHandle, attr: YacConnAttr, v: u32) -> Result<(), Error> {
        self.set_conn_attr(
            dbc,
            attr,
            &v as *const u32 as *mut c_void,
            std::mem::size_of::<u32>() as i32,
        )
    }

    /// Get a connection attribute as an `i32`.
    #[inline]
    fn get_conn_attr_i32(&self, dbc: &mut DbcHandle, attr: YacConnAttr) -> Result<i32, Error> {
        let mut v: i32 = 0;
        let mut string_length: YacInt32 = 0;
        self.get_conn_attr(
            dbc,
            attr,
            &mut v as *mut i32 as *mut c_void,
            std::mem::size_of::<i32>() as i32,
            &mut string_length,
        )?;
        Ok(v)
    }

    /// Get a connection attribute as a `u32`.
    #[inline]
    fn get_conn_attr_u32(&self, dbc: &mut DbcHandle, attr: YacConnAttr) -> Result<u32, Error> {
        let mut v: u32 = 0;
        let mut string_length: YacInt32 = 0;
        self.get_conn_attr(
            dbc,
            attr,
            &mut v as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as i32,
            &mut string_length,
        )?;
        Ok(v)
    }

    // --- env attribute setters ---

    /// Set the client character set (`YAC_ATTR_CHARSET_CODE`).
    ///
    /// Supported by the baseline client library, so this cannot fail at runtime
    /// for a well-formed argument.
    #[inline]
    pub fn set_env_charset_code(&self, env: &mut EnvHandle, charset: YacCharsetCode) {
        self.set_env_attr(
            env,
            YacEnvAttr::CharsetCode,
            &mut (charset as i32) as *mut i32 as *mut c_void,
            size_of::<i32>() as i32,
        )
        .expect("yacSetEnvAttr(YAC_ATTR_CHARSET_CODE) failed")
    }

    /// Set the client driver name reported to the server (`YAC_ATTR_CLIENT_DRIVER`).
    #[inline]
    pub fn set_env_client_driver(&self, env: &mut EnvHandle, name: &str) {
        self.set_env_attr(
            env,
            YacEnvAttr::ClientDriver,
            name.as_ptr() as *mut c_void,
            name.len() as i32,
        )
        .expect("yacSetEnvAttr(YAC_ATTR_CLIENT_DRIVER) failed")
    }

    /// Set the client software version reported to the server (`YAC_ATTR_SOFTWARE_VERSION`).
    #[inline]
    pub fn set_env_software_version(&self, env: &mut EnvHandle, version: &str) {
        self.set_env_attr(
            env,
            YacEnvAttr::SoftwareVersion,
            version.as_ptr() as *mut c_void,
            version.len() as i32,
        )
        .expect("yacSetEnvAttr(YAC_ATTR_SOFTWARE_VERSION) failed")
    }

    // --- connection attribute setters ---

    /// Set the auto-commit mode (`YAC_ATTR_AUTOCOMMIT`).
    ///
    /// Supported by the baseline client library; this cannot fail at runtime.
    #[inline]
    pub fn set_conn_auto_commit(&self, dbc: &mut DbcHandle, enabled: bool) {
        self.set_conn_attr_i32(dbc, YacConnAttr::AutoCommit, if enabled { 1 } else { 0 })
            .expect("yacSetConnAttr(YAC_ATTR_AUTOCOMMIT) failed")
    }

    /// Set whether heartbeat is enabled (`YAC_ATTR_HEARTBEAT_ENABLED`).
    ///
    /// Supported by the baseline client library; this cannot fail at runtime.
    #[inline]
    pub fn set_conn_heartbeat_enabled(&self, dbc: &mut DbcHandle, enabled: bool) {
        self.set_conn_attr_i32(dbc, YacConnAttr::HeartbeatEnabled, if enabled { 1 } else { 0 })
            .expect("yacSetConnAttr(YAC_ATTR_HEARTBEAT_ENABLED) failed")
    }

    /// Set the transaction isolation level (`YAC_ATTR_TXN_ISOLATION`).
    ///
    /// This sends a message to the server and may fail if the session is in a
    /// transaction.
    #[inline]
    pub fn set_conn_transaction_isolation(&self, dbc: &mut DbcHandle, level: YacTxnIsolation) -> Result<(), Error> {
        self.set_conn_attr_i32(dbc, YacConnAttr::TransactionIsolation, level as i32)
    }

    /// Set the login timeout in seconds (`YAC_ATTR_LOGIN_TIMEOUT`).
    ///
    /// Not supported by the baseline client library (attr 4 reports
    /// `unknown attribute id`), so the error is returned rather than panicking.
    #[inline]
    pub fn set_conn_login_timeout(&self, dbc: &mut DbcHandle, seconds: u32) -> Result<(), Error> {
        self.set_conn_attr_u32(dbc, YacConnAttr::LoginTimeout, seconds)
    }

    /// Set the packet size in bytes (`YAC_ATTR_PACKET_SIZE`).
    ///
    /// Supported by the baseline client library; this cannot fail at runtime.
    #[inline]
    pub fn set_conn_packet_size(&self, dbc: &mut DbcHandle, bytes: u32) {
        self.set_conn_attr_u32(dbc, YacConnAttr::PacketSize, bytes)
            .expect("yacSetConnAttr(YAC_ATTR_PACKET_SIZE) failed")
    }

    // --- connection attribute getters ---

    /// Get the auto-commit mode (`YAC_ATTR_AUTOCOMMIT`).
    ///
    /// Supported by the baseline client library; this cannot fail at runtime.
    #[inline]
    pub fn get_conn_auto_commit(&self, dbc: &mut DbcHandle) -> bool {
        self.get_conn_attr_i32(dbc, YacConnAttr::AutoCommit)
            .expect("yacGetConnAttr(YAC_ATTR_AUTOCOMMIT) failed")
            != 0
    }

    /// Get whether heartbeat is enabled (`YAC_ATTR_HEARTBEAT_ENABLED`).
    ///
    /// Supported by the baseline client library; this cannot fail at runtime.
    #[inline]
    pub fn get_conn_heartbeat_enabled(&self, dbc: &mut DbcHandle) -> bool {
        self.get_conn_attr_i32(dbc, YacConnAttr::HeartbeatEnabled)
            .expect("yacGetConnAttr(YAC_ATTR_HEARTBEAT_ENABLED) failed")
            != 0
    }

    /// Get the transaction isolation level (`YAC_ATTR_TXN_ISOLATION`).
    ///
    /// Supported by the baseline client library. Panics only if the driver
    /// returns an unrecognized isolation level (ffi enum mismatch: a developer
    /// error).
    #[inline]
    pub fn get_conn_transaction_isolation(&self, dbc: &mut DbcHandle) -> YacTxnIsolation {
        let v = self
            .get_conn_attr_i32(dbc, YacConnAttr::TransactionIsolation)
            .expect("yacGetConnAttr(YAC_ATTR_TXN_ISOLATION) failed");
        match v {
            v if v == YacTxnIsolation::ReadCommitted as i32 => YacTxnIsolation::ReadCommitted,
            v if v == YacTxnIsolation::CurrCommitted as i32 => YacTxnIsolation::CurrCommitted,
            v if v == YacTxnIsolation::Serializable as i32 => YacTxnIsolation::Serializable,
            other => panic!("yacGetConnAttr(YAC_ATTR_TXN_ISOLATION) returned unknown level: {other}"),
        }
    }

    /// Get the login timeout in seconds (`YAC_ATTR_LOGIN_TIMEOUT`).
    #[allow(dead_code)]
    #[inline]
    pub fn get_conn_login_timeout(&self, dbc: &mut DbcHandle) -> u32 {
        self.get_conn_attr_u32(dbc, YacConnAttr::LoginTimeout)
            .expect("yacGetConnAttr(YAC_ATTR_LOGIN_TIMEOUT) failed")
    }

    /// Get the packet size in bytes (`YAC_ATTR_PACKET_SIZE`).
    ///
    /// Supported by the baseline client library; this cannot fail at runtime.
    #[inline]
    pub fn get_conn_packet_size(&self, dbc: &mut DbcHandle) -> u32 {
        self.get_conn_attr_u32(dbc, YacConnAttr::PacketSize)
            .expect("yacGetConnAttr(YAC_ATTR_PACKET_SIZE) failed")
    }

    /// Get the maximum bytes per database character for the current connection.
    #[inline]
    pub fn get_conn_max_charset_ratio(&self, dbc: &mut DbcHandle) -> Result<u32, Error> {
        self.get_conn_attr_u32(dbc, YacConnAttr::MaxCharsetRatio)
    }

    /// Get the maximum bytes per national character for the current connection.
    #[inline]
    pub fn get_conn_max_ncharset_ratio(&self, dbc: &mut DbcHandle) -> Result<u32, Error> {
        self.get_conn_attr_u32(dbc, YacConnAttr::MaxNcharsetRatio)
    }
}
