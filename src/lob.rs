//! Safe connection-bound BLOB and character LOB handles.

use crate::conn::Connection;
use crate::error::Error;
use crate::ffi::{LobLocator, YacTempLobType};

pub(crate) struct Lob<'conn> {
    conn: &'conn Connection,
    locator: Option<LobLocator>,
}

impl<'conn> Lob<'conn> {
    #[inline]
    pub(crate) fn new(conn: &'conn Connection) -> Result<Self, Error> {
        Ok(Self {
            conn,
            locator: Some(conn.with_handle(|lib, dbc| lib.lob_desc_alloc(dbc))?),
        })
    }

    #[inline]
    pub(crate) fn take(&mut self) -> Option<Self> {
        self.locator.take().map(|locator| Self {
            conn: self.conn,
            locator: Some(locator),
        })
    }

    #[inline]
    pub(crate) fn is_live(&self) -> bool {
        self.locator.is_some()
    }

    #[inline]
    fn locator(&self) -> Result<&LobLocator, Error> {
        self.locator
            .as_ref()
            .ok_or_else(|| Error::Internal("LOB locator was already released".into()))
    }

    #[inline]
    fn with_handle<T>(
        &self,
        operation: impl FnOnce(&crate::ffi::YacLib, &mut crate::ffi::DbcHandle, &LobLocator) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let locator = self.locator()?;
        self.conn.with_handle(|lib, dbc| operation(lib, dbc, locator))
    }

    #[inline]
    fn create_temporary(&self, lob_type: YacTempLobType) -> Result<(), Error> {
        self.with_handle(|lib, dbc, locator| lib.lob_create_temporary(dbc, locator, lob_type))
    }

    #[inline]
    fn free_temporary(&self) -> Result<(), Error> {
        self.with_handle(|lib, dbc, locator| lib.lob_free_temporary(dbc, locator))
    }

    #[inline]
    fn is_temporary(&self) -> bool {
        let locator = self.locator().expect("live LOB locator");
        self.conn.with_handle(|lib, dbc| lib.lob_is_temporary(dbc, locator))
    }

    #[inline]
    fn free_temporary_if_needed(&self) -> Result<(), Error> {
        if self.locator.is_none() {
            return Ok(());
        }
        // The locator state changes after yacLobFreeTemporary, so repeated
        // cleanup attempts, including the Drop fallback, are harmless no-ops.
        if self.is_temporary() {
            self.free_temporary()
        } else {
            Ok(())
        }
    }

    #[inline]
    pub(crate) fn bind_output(&mut self) -> &mut [u8] {
        self.locator.as_mut().expect("live LOB locator").bind_output()
    }
}

impl Drop for Lob<'_> {
    #[inline]
    fn drop(&mut self) {
        // The locator tracks whether it still represents a temporary LOB. This
        // is a best-effort fallback when explicit Blob/Clob cleanup was skipped
        // or failed; a successful explicit cleanup makes this a no-op.
        let _ = self.free_temporary_if_needed();

        // Descriptor ownership is separate from the server-side temporary LOB.
        // Drop always releases the client-side descriptor.
        if let Some(locator) = self.locator.take() {
            self.conn.lib().lob_desc_free(locator);
        }
    }
}

/// A connection-bound binary large object locator.
pub struct Blob<'conn> {
    lob: Lob<'conn>,
}

impl<'conn> Blob<'conn> {
    #[inline]
    pub(crate) fn output(conn: &'conn Connection) -> Result<Self, Error> {
        Ok(Self { lob: Lob::new(conn)? })
    }

    #[inline]
    pub(crate) fn from_lob(lob: Lob<'conn>) -> Self {
        Self { lob }
    }

    #[inline]
    pub(crate) fn temporary(conn: &'conn Connection) -> Result<Self, Error> {
        let lob = Lob::new(conn)?;
        lob.create_temporary(YacTempLobType::Blob)?;
        Ok(Self { lob })
    }

    /// Return the BLOB length in bytes.
    #[inline]
    pub fn len(&self) -> Result<u64, Error> {
        self.lob.with_handle(|lib, dbc, locator| lib.lob_length(dbc, locator))
    }

    /// Return whether this BLOB is empty.
    #[inline]
    pub fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }

    /// Return whether this BLOB locator currently refers to a temporary LOB.
    #[inline]
    pub fn is_temporary(&self) -> bool {
        self.lob.is_temporary()
    }

    /// Return the client-recommended transfer chunk size in bytes.
    #[inline]
    pub fn chunk_size(&self) -> Result<usize, Error> {
        Ok(self
            .lob
            .with_handle(|lib, dbc, locator| lib.lob_chunk_size(dbc, locator))? as usize)
    }

    /// Truncate this BLOB to `len` bytes.
    #[inline]
    pub fn truncate(&mut self, len: u64) -> Result<(), Error> {
        self.lob
            .with_handle(|lib, dbc, locator| lib.lob_trim(dbc, locator, len))
    }

    /// Read from a one-based byte offset into `buffer`.
    #[inline]
    pub fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<usize, Error> {
        if offset == 0 {
            return Err(Error::InvalidArgument("LOB offset must start at 1".into()));
        }
        if buffer.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self.lob.with_handle(|lib, dbc, locator| unsafe {
            // SAFETY: `buffer` is a valid mutable Rust slice for the duration of
            // the call. The FFI wrapper passes its pointer and length to the C
            // driver, which writes at most that many bytes.
            lib.lob_read(
                dbc,
                locator,
                offset,
                buffer.len() as u64,
                0,
                std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast(), buffer.len()),
            )
        })?;
        Ok(bytes as usize)
    }

    /// Write to a one-based byte offset.
    #[inline]
    pub fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<usize, Error> {
        if offset == 0 {
            return Err(Error::InvalidArgument("LOB offset must start at 1".into()));
        }
        if data.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self
            .lob
            .with_handle(|lib, dbc, locator| lib.lob_write(dbc, locator, offset, data.len() as u64, 0, data))?;
        Ok(bytes as usize)
    }

    /// Append bytes and return the number written.
    #[inline]
    pub fn append(&mut self, data: &[u8]) -> Result<usize, Error> {
        if data.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self
            .lob
            .with_handle(|lib, dbc, locator| lib.lob_write_append(dbc, locator, data.len() as u64, 0, data))?;
        Ok(bytes as usize)
    }

    /// Read the complete BLOB and append it to `output`.
    pub fn read_to_end(&mut self, output: &mut Vec<u8>) -> Result<(), Error> {
        let len = self.len()?;
        if len > isize::MAX as u64 {
            return Err(Error::LobTooLarge { length: len });
        }
        let capacity = len as usize;

        let output_len = output.len();
        output.reserve(capacity);

        let chunk_size = self.chunk_size()?.max(1);
        let mut offset = 1;
        let mut filled = 0;

        while filled < capacity {
            let remaining = capacity - filled;
            let chunk_len = remaining.min(chunk_size);
            let spare = &mut output.spare_capacity_mut()[..chunk_len];

            let (bytes, _) = self
                .lob
                .with_handle(|lib, dbc, locator| lib.lob_read(dbc, locator, offset, chunk_len as u64, 0, spare))?;
            let count = bytes as usize;
            if count == 0 {
                break;
            }

            // `lob_read` verifies that the driver wrote within `spare`.
            // SAFETY: The client initialized exactly `count` bytes in the spare
            // capacity, and `lob_read` checked that `count` fits in the slice.
            unsafe { output.set_len(output_len + filled + count) };
            filled += count;
            offset += count as u64;

            if count < chunk_len {
                break;
            }
        }

        Ok(())
    }

    /// Explicitly release the locator and report cleanup errors.
    #[inline]
    pub fn finish(mut self) -> Result<(), Error> {
        self.cleanup()
    }

    #[inline]
    pub(crate) fn bind_input(&self) -> &[u8] {
        self.lob.locator.as_ref().expect("live LOB").bind_input()
    }

    #[inline]
    pub(crate) fn bind_output(&mut self) -> &mut [u8] {
        self.lob.bind_output()
    }

    #[inline]
    fn cleanup(&mut self) -> Result<(), Error> {
        // Do not free the descriptor here: Lob::drop owns that cleanup path.
        // The locator state prevents Drop from freeing a temporary LOB twice.
        self.lob.free_temporary_if_needed()
    }
}

impl Drop for Blob<'_> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// A connection-bound character large object locator.
pub struct Clob<'conn> {
    lob: Lob<'conn>,
    lob_type: YacTempLobType,
}

impl<'conn> Clob<'conn> {
    #[inline]
    pub(crate) fn output(conn: &'conn Connection) -> Result<Self, Error> {
        Ok(Self {
            lob: Lob::new(conn)?,
            lob_type: YacTempLobType::Clob,
        })
    }

    #[inline]
    pub(crate) fn from_lob(lob: Lob<'conn>, lob_type: YacTempLobType) -> Self {
        Self { lob, lob_type }
    }

    #[inline]
    pub(crate) fn temporary(conn: &'conn Connection) -> Result<Self, Error> {
        let lob = Lob::new(conn)?;
        lob.create_temporary(YacTempLobType::Clob)?;
        Ok(Self {
            lob,
            lob_type: YacTempLobType::Clob,
        })
    }

    /// Return the text LOB length in characters.
    #[inline]
    pub fn len(&self) -> Result<u64, Error> {
        self.lob.with_handle(|lib, dbc, locator| lib.lob_length(dbc, locator))
    }

    /// Return whether this text LOB is empty.
    #[inline]
    pub fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }

    /// Return whether this text LOB locator currently refers to a temporary LOB.
    #[inline]
    pub fn is_temporary(&self) -> bool {
        self.lob.is_temporary()
    }

    #[inline]
    fn charset_ratio(&self) -> Result<u64, Error> {
        debug_assert!(matches!(self.lob_type, YacTempLobType::Clob | YacTempLobType::NClob));
        Ok(match self.lob_type {
            YacTempLobType::Clob => self.lob.conn.charset_ratios()?.0,
            // NCLOB uses UTF-16: BMP characters occupy 2 bytes, while
            // non-BMP characters occupy a 4-byte surrogate pair. The client
            // reported national charset ratio is fixed at 2, so use 4 here.
            YacTempLobType::NClob => 4,
            YacTempLobType::Blob => unreachable!("Clob cannot contain a BLOB locator"),
        }
        .max(1) as u64)
    }

    /// Return the client-recommended transfer chunk size in bytes.
    #[inline]
    pub fn chunk_size(&self) -> Result<usize, Error> {
        Ok(self
            .lob
            .with_handle(|lib, dbc, locator| lib.lob_chunk_size(dbc, locator))? as usize)
    }

    /// Truncate this text LOB to a character length.
    #[inline]
    pub fn truncate(&mut self, len: u64) -> Result<(), Error> {
        self.lob
            .with_handle(|lib, dbc, locator| lib.lob_trim(dbc, locator, len))
    }

    /// Read up to `max_chars` characters from a one-based character offset into `output`.
    pub fn read_at(&self, offset: u64, max_chars: u64, output: &mut String) -> Result<u64, Error> {
        if offset == 0 {
            return Err(Error::InvalidArgument("LOB offset must start at 1".into()));
        }
        if max_chars == 0 {
            return Ok(0);
        }

        let ratio = self.charset_ratio()?;
        let bytes = max_chars * ratio;
        if bytes > isize::MAX as u64 {
            return Err(Error::LobTooLarge { length: max_chars });
        }
        let size = bytes as usize;
        let output_len = output.len();
        output.reserve(size);

        // SAFETY: `size` bytes are reserved immediately below. The returned
        // spare-capacity slice is written only by the client during `lob_read`.
        let spare = unsafe { &mut output.as_mut_vec().spare_capacity_mut()[..size] };
        let (byte_count, char_count) = self
            .lob
            .with_handle(|lib, dbc, locator| lib.lob_read(dbc, locator, offset, bytes, max_chars, spare))?;
        let byte_count = byte_count as usize;
        // SAFETY: `lob_read` checked that `byte_count` fits in `spare` and the C
        // driver guarantees that text LOB bytes returned here are valid UTF-8.
        unsafe { output.as_mut_vec().set_len(output_len + byte_count) };
        Ok(char_count)
    }

    /// Write UTF-8 text at a one-based character offset.
    #[inline]
    pub fn write_at(&mut self, offset: u64, value: &str) -> Result<u64, Error> {
        if offset == 0 {
            return Err(Error::InvalidArgument("LOB offset must start at 1".into()));
        }
        self.lob
            .with_handle(|lib, dbc, locator| {
                lib.lob_write(
                    dbc,
                    locator,
                    offset,
                    value.len() as u64,
                    value.chars().count() as u64,
                    value.as_bytes(),
                )
            })
            .map(|(_, chars)| chars)
    }

    /// Append UTF-8 text and return the number of characters written.
    #[inline]
    pub fn append(&mut self, value: &str) -> Result<u64, Error> {
        self.lob
            .with_handle(|lib, dbc, locator| {
                lib.lob_write_append(
                    dbc,
                    locator,
                    value.len() as u64,
                    value.chars().count() as u64,
                    value.as_bytes(),
                )
            })
            .map(|(_, chars)| chars)
    }

    /// Read the complete text LOB into `output`.
    pub fn read_to_string(&mut self, output: &mut String) -> Result<(), Error> {
        let len = self.len()?;
        let ratio = self.charset_ratio()?;
        let bytes = len * ratio;
        if bytes > isize::MAX as u64 {
            return Err(Error::LobTooLarge { length: len });
        }
        output.reserve(bytes as usize);

        let chunk_chars = (self.chunk_size()? as u64 / ratio).max(1);
        let mut offset = 1;
        while offset <= len {
            let count = self.read_at(offset, chunk_chars, output)?;
            if count == 0 {
                break;
            }
            offset += count;
        }
        Ok(())
    }

    /// Explicitly release the locator and report cleanup errors.
    #[inline]
    pub fn finish(mut self) -> Result<(), Error> {
        self.cleanup()
    }

    #[inline]
    pub(crate) fn bind_input(&self) -> &[u8] {
        self.lob.locator.as_ref().expect("live LOB").bind_input()
    }

    #[inline]
    pub(crate) fn bind_output(&mut self) -> &mut [u8] {
        self.lob.bind_output()
    }

    #[inline]
    fn cleanup(&mut self) -> Result<(), Error> {
        // Do not free the descriptor here: Lob::drop owns that cleanup path.
        // The locator state prevents Drop from freeing a temporary LOB twice.
        self.lob.free_temporary_if_needed()
    }
}

impl Drop for Clob<'_> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
