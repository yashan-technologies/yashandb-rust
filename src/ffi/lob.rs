//! ABI wrappers for the second-generation YACLI LOB APIs.

use std::mem::MaybeUninit;
use std::ptr::NonNull;

use crate::error::Error;
use crate::ffi::raw::*;
use crate::ffi::{DbcHandle, YacLib};

/// Owned non-null YACLI LOB locator descriptor.
#[repr(transparent)]
pub struct LobLocator(NonNull<YacLobLocator>);

impl LobLocator {
    #[inline]
    pub(crate) fn bind_input(&self) -> &[u8] {
        // YACLI's LOB bind value is the address of a `YacLobLocator *` variable
        // (`YacLobLocator **`), not the address of the locator contents. This
        // byte view therefore passes `&self.0`, matching the C example's
        // `&lobLocator` argument. The caller keeps the variable alive for the
        // complete bind call.
        //
        // SAFETY: The slice is only a byte view of this pointer-sized field.
        unsafe {
            std::slice::from_raw_parts(
                (&self.0 as *const NonNull<YacLobLocator>).cast(),
                size_of::<*mut YacLobLocator>(),
            )
        }
    }

    #[inline]
    pub(crate) fn bind_output(&mut self) -> &mut [u8] {
        // As with input binding, YACLI expects the address of the
        // `YacLobLocator *` variable (`YacLobLocator **`). The client operates
        // on this pointer variable; it does not write a locator structure into
        // the Rust field and does not replace the descriptor allocation here.
        //
        // SAFETY: The slice is only a mutable byte view of this pointer-sized
        // field, and the field remains valid for the complete bind call.
        unsafe {
            std::slice::from_raw_parts_mut(
                (&mut self.0 as *mut NonNull<YacLobLocator>).cast(),
                size_of::<*mut YacLobLocator>(),
            )
        }
    }

    #[inline]
    fn as_ptr(&self) -> *mut YacLobLocator {
        self.0.as_ptr()
    }
}

impl YacLib {
    #[inline]
    pub fn lob_desc_alloc(&self, dbc: &mut DbcHandle) -> Result<LobLocator, Error> {
        let mut locator = std::ptr::null_mut();
        self.try_call(|| unsafe { (self.lob_desc_alloc2)(dbc.as_ptr(), &mut locator) })?;
        if locator.is_null() {
            return Err(Error::ClientLibrary("yacLobDescAlloc2 returned a null locator".into()));
        }
        Ok(LobLocator(NonNull::new(locator).expect("checked above")))
    }

    #[inline]
    pub fn lob_desc_free(&self, locator: LobLocator) {
        // yacLobDescFree2 releases only client-owned descriptor memory. YACLI
        // documents this operation as infallible, so its return code is ignored.
        let result = unsafe { (self.lob_desc_free2)(locator.as_ptr()) };
        assert_eq!(result, YacResult::Success, "yacLobDescFree2 failed");
    }

    #[inline]
    pub fn lob_create_temporary(
        &self,
        dbc: &mut DbcHandle,
        locator: &LobLocator,
        lob_type: YacTempLobType,
    ) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.lob_create_temporary2)(dbc.as_ptr(), locator.as_ptr(), lob_type) })
    }

    #[inline]
    pub fn lob_free_temporary(&self, dbc: &mut DbcHandle, locator: &LobLocator) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.lob_free_temporary)(dbc.as_ptr(), locator.as_ptr()) })
    }

    #[inline]
    pub fn lob_is_temporary(&self, dbc: &mut DbcHandle, locator: &LobLocator) -> bool {
        let mut is_temporary = 0;
        // YACLI reports the locator's current state through `is_temporary`; the
        // return code is informational for this state query and is ignored.
        let result = unsafe { (self.lob_is_temporary)(dbc.as_ptr(), locator.as_ptr(), &mut is_temporary) };
        assert_eq!(result, YacResult::Success, "yacLobIsTemporary failed");
        is_temporary != 0
    }

    #[inline]
    pub fn lob_length(&self, dbc: &mut DbcHandle, locator: &LobLocator) -> Result<u64, Error> {
        let mut length = 0;
        self.try_call(|| unsafe { (self.lob_get_length)(dbc.as_ptr(), locator.as_ptr(), &mut length) })?;
        Ok(length)
    }

    #[inline]
    pub fn lob_chunk_size(&self, dbc: &mut DbcHandle, locator: &LobLocator) -> Result<u16, Error> {
        let mut size = 0;
        self.try_call(|| unsafe { (self.lob_get_chunk_size)(dbc.as_ptr(), locator.as_ptr(), &mut size) })?;
        Ok(size)
    }

    #[inline]
    pub fn lob_trim(&self, dbc: &mut DbcHandle, locator: &LobLocator, length: u64) -> Result<(), Error> {
        let mut length = length;
        self.try_call(|| unsafe { (self.lob_trim)(dbc.as_ptr(), locator.as_ptr(), &mut length) })
    }

    #[inline]
    pub fn lob_read(
        &self,
        dbc: &mut DbcHandle,
        locator: &LobLocator,
        offset: u64,
        byte_size: u64,
        char_size: u64,
        buffer: &mut [MaybeUninit<u8>],
    ) -> Result<(u64, u64), Error> {
        let mut bytes = byte_size;
        let mut chars = char_size;
        self.try_call(|| unsafe {
            (self.lob_read2)(
                dbc.as_ptr(),
                locator.as_ptr(),
                &mut bytes,
                &mut chars,
                offset,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u64,
            )
        })?;
        if bytes > buffer.len() as u64 {
            return Err(Error::Internal(
                "yacLobRead2 returned more bytes than the supplied buffer".into(),
            ));
        }
        Ok((bytes, chars))
    }

    #[inline]
    pub fn lob_write(
        &self,
        dbc: &mut DbcHandle,
        locator: &LobLocator,
        offset: u64,
        byte_size: u64,
        char_size: u64,
        buffer: &[u8],
    ) -> Result<(u64, u64), Error> {
        let mut bytes = byte_size;
        let mut chars = char_size;
        self.try_call(|| unsafe {
            (self.lob_write2)(
                dbc.as_ptr(),
                locator.as_ptr(),
                &mut bytes,
                &mut chars,
                offset,
                buffer.as_ptr().cast_mut(),
                buffer.len() as u64,
            )
        })?;
        Ok((bytes, chars))
    }

    #[inline]
    pub fn lob_write_append(
        &self,
        dbc: &mut DbcHandle,
        locator: &LobLocator,
        byte_size: u64,
        char_size: u64,
        buffer: &[u8],
    ) -> Result<(u64, u64), Error> {
        let mut bytes = byte_size;
        let mut chars = char_size;
        self.try_call(|| unsafe {
            (self.lob_write_append)(
                dbc.as_ptr(),
                locator.as_ptr(),
                &mut bytes,
                &mut chars,
                buffer.as_ptr().cast_mut(),
                buffer.len() as u64,
            )
        })?;
        Ok((bytes, chars))
    }
}
