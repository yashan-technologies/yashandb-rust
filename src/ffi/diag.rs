//! Database error diagnostics via `yacGetDiagRec`.

use std::ffi::CStr;
use std::mem::MaybeUninit;

use crate::error::Error;
use crate::ffi::YacLib;
use crate::ffi::raw::*;

impl YacLib {
    /// Call `yacGetDiagRec` and return a populated `Error::Database`.
    #[cold]
    pub(super) fn get_diag_rec(&self) -> Error {
        const MESSAGE_BUF_LEN: usize = 8192;

        let mut code: YacInt32 = 0;
        let mut message: [MaybeUninit<u8>; MESSAGE_BUF_LEN] = [MaybeUninit::uninit(); MESSAGE_BUF_LEN];
        let mut indicator: YacInt32 = 0;
        let mut pos = YacTextPos { line: 0, column: 0 };

        let rc = unsafe {
            (self.get_diag_rec)(
                &mut code,
                message.as_mut_ptr() as *mut u8,
                message.len() as YacInt32,
                &mut indicator,
                std::ptr::null_mut(),
                0,
                &mut pos,
            )
        };

        if rc == YacResult::Success || rc == YacResult::SuccessWithInfo {
            // `indicator` reports the actual message length written by the driver
            // (it may exceed the buffer when the message is truncated). Clamp to
            // the buffer bounds so we never scan uninitialized bytes.
            let len = (indicator.max(0) as usize).min(message.len());
            let bytes = unsafe { std::slice::from_raw_parts(message.as_ptr() as *const u8, len) };
            // The connection always sets the env charset to UTF-8 (`set_env_attrs`
            // in conn.rs), so decoding as UTF-8 is correct.
            let msg = match CStr::from_bytes_until_nul(bytes) {
                Ok(cstr) => cstr.to_string_lossy().into_owned(),
                // Length excludes the NUL terminator: use the bytes as-is.
                Err(_) => String::from_utf8_lossy(bytes).into_owned(),
            };
            Error::Database {
                code,
                message: msg,
                line: pos.line,
                column: pos.column,
            }
        } else {
            Error::Database {
                code: -1,
                message: "failed to retrieve diagnostic info".to_string(),
                line: 0,
                column: 0,
            }
        }
    }
}
