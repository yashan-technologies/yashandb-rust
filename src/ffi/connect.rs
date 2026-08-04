//! Handle lifecycle: allocate, free, connect, disconnect.

use std::ffi::c_void;
use std::ptr::NonNull;

use crate::error::Error;
use crate::ffi::YacLib;
use crate::ffi::raw::*;

/// Wrapper around a raw env handle (`YacHandleType::Env`).
#[repr(transparent)]
pub struct EnvHandle(NonNull<c_void>);

/// Wrapper around a raw dbc handle (`YacHandleType::Dbc`).
#[repr(transparent)]
pub struct DbcHandle(NonNull<c_void>);

impl YacLib {
    // Handles below take `&mut` even when only read: the `&mut` borrow guarantees
    // exclusive access, so the same C handle is never touched concurrently from
    // multiple threads. A shared `&` would allow concurrent access via `&env`.
    // (Connection is `Send + !Sync`; this is what makes that sound.)

    /// Allocate an env handle.
    #[inline]
    pub fn alloc_env(&self) -> Result<EnvHandle, Error> {
        let mut output = std::ptr::null_mut();
        self.try_call(|| unsafe { (self.alloc_handle)(YacHandleType::Env, std::ptr::null_mut(), &mut output) })?;
        let handle = NonNull::new(output)
            .ok_or_else(|| Error::ClientLibrary("yacAllocHandle returned a null env handle".to_string()))?;
        Ok(EnvHandle(handle))
    }

    /// Free an env handle.
    #[inline]
    pub fn free_env(&self, env: &mut EnvHandle) {
        let rc = unsafe { (self.free_handle)(YacHandleType::Env, env.0.as_ptr()) };
        // yacFreeHandle only fails on a handle-type mismatch, and we free with
        // the same type used to allocate, so failure is not expected here.
        debug_assert_eq!(rc, YacResult::Success, "yacFreeHandle(ENV) failed");
    }

    /// Allocate a dbc handle under the given env handle.
    #[inline]
    pub fn alloc_dbc(&self, env: &mut EnvHandle) -> Result<DbcHandle, Error> {
        let mut output = std::ptr::null_mut();
        self.try_call(|| unsafe { (self.alloc_handle)(YacHandleType::Dbc, env.0.as_ptr(), &mut output) })?;
        let handle = NonNull::new(output)
            .ok_or_else(|| Error::ClientLibrary("yacAllocHandle returned a null dbc handle".to_string()))?;
        Ok(DbcHandle(handle))
    }

    /// Free a dbc handle.
    #[inline]
    pub fn free_dbc(&self, dbc: &mut DbcHandle) {
        let rc = unsafe { (self.free_handle)(YacHandleType::Dbc, dbc.0.as_ptr()) };
        // yacFreeHandle only fails on a handle-type mismatch, and we free with
        // the same type used to allocate, so failure is not expected here.
        debug_assert_eq!(rc, YacResult::Success, "yacFreeHandle(DBC) failed");
    }

    /// Establish a connection to the YashanDB instance.
    #[inline]
    pub fn connect(&self, dbc: &mut DbcHandle, url: &str, username: &str, password: &str) -> Result<(), Error> {
        // TODO(attr): the C driver defaults to GBK; UTF-8 bytes passed here will
        // be garbled for non-ASCII input. Set `YacEnvAttr::CharsetCode` (or
        // convert to GBK) before connecting once the attribute API lands.
        let url_len = conn_param_len(url)?;
        let username_len = conn_param_len(username)?;
        let password_len = conn_param_len(password)?;
        self.try_call(|| unsafe {
            (self.connect)(
                dbc.0.as_ptr(),
                url.as_ptr(),
                url_len,
                username.as_ptr(),
                username_len,
                password.as_ptr(),
                password_len,
            )
        })
    }

    /// Disconnect from the YashanDB instance.
    #[inline]
    pub fn disconnect(&self, dbc: &mut DbcHandle) {
        // yacDisconnect returns void; the C driver handles the disconnect internally.
        unsafe { (self.disconnect)(dbc.0.as_ptr()) };
    }
}

/// Convert a connection parameter length to the C `YacInt16` length field,
/// rejecting strings that do not fit (the C driver would misinterpret them).
#[inline]
fn conn_param_len(s: &str) -> Result<YacInt16, Error> {
    YacInt16::try_from(s.len()).map_err(|_| {
        Error::InvalidArgument(format!(
            "connection parameter exceeds the maximum of {max} bytes",
            max = YacInt16::MAX
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conn_param_len_empty() {
        assert_eq!(conn_param_len("").unwrap(), 0);
    }

    #[test]
    fn conn_param_len_ascii() {
        assert_eq!(conn_param_len("127.0.0.1:1688").unwrap(), 14);
    }

    #[test]
    fn conn_param_len_utf8_multibyte() {
        // Length is measured in bytes, not chars.
        assert_eq!(conn_param_len("雪").unwrap(), 3);
        assert_eq!(conn_param_len("用户").unwrap(), 6);
    }

    #[test]
    fn conn_param_len_max_ok() {
        let max = i16::MAX as usize;
        let s = "a".repeat(max);
        assert_eq!(conn_param_len(&s).unwrap(), max as i16);
    }

    #[test]
    fn conn_param_len_overflow_errors() {
        let max = i16::MAX as usize;
        let s = "a".repeat(max + 1);
        let err = conn_param_len(&s).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)));
        assert_stream_max(&err);
    }

    fn assert_stream_max(err: &Error) {
        let Error::InvalidArgument(msg) = err else {
            panic!("expected InvalidArgument, got {err:?}");
        };
        assert!(msg.contains("maximum"), "unexpected message: {msg}");
    }
}
