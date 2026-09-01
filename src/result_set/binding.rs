//! Native storage bound to result columns.

use std::cell::RefCell;
use std::mem::MaybeUninit;

use crate::lob::Lob;
use crate::types::{Date, IntervalDS, IntervalYM, Time, YacNumber, YacTimestamp};

/// Storage bound to a result column for one fetched row.
///
/// Each variant owns storage with the Rust representation matching its database
/// type. Text and binary values retain uninitialized storage because the client
/// writes their variable-length contents during fetch.
pub(super) enum ColumnBinding<'conn> {
    Unsupported,
    Bool(u8),
    TinyInt(i8),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Float(f32),
    Double(f64),
    Number(YacNumber),
    Date(Date),
    ShortTime(Time),
    Timestamp(YacTimestamp),
    IntervalYM(IntervalYM),
    IntervalDS(IntervalDS),
    Text(Vec<MaybeUninit<u8>>),
    Binary(Vec<MaybeUninit<u8>>),
    Lob(RefCell<Lob<'conn>>),
}

impl ColumnBinding<'_> {
    #[inline]
    fn scalar_buffer<T>(value: &mut T) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(std::ptr::from_mut(value).cast(), size_of_val(value)) }
    }

    #[inline]
    pub(super) fn bool_buffer(&mut self) -> &mut [u8] {
        let Self::Bool(value) = self else {
            unreachable!("BOOL binding accessor requires a bool binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn tiny_int_buffer(&mut self) -> &mut [u8] {
        let Self::TinyInt(value) = self else {
            unreachable!("TINYINT binding accessor requires an i8 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn small_int_buffer(&mut self) -> &mut [u8] {
        let Self::SmallInt(value) = self else {
            unreachable!("SMALLINT binding accessor requires an i16 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn integer_buffer(&mut self) -> &mut [u8] {
        let Self::Integer(value) = self else {
            unreachable!("INTEGER binding accessor requires an i32 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn big_int_buffer(&mut self) -> &mut [u8] {
        let Self::BigInt(value) = self else {
            unreachable!("BIGINT binding accessor requires an i64 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn float_buffer(&mut self) -> &mut [u8] {
        let Self::Float(value) = self else {
            unreachable!("FLOAT binding accessor requires an f32 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn double_buffer(&mut self) -> &mut [u8] {
        let Self::Double(value) = self else {
            unreachable!("DOUBLE binding accessor requires an f64 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn number_buffer(&mut self) -> &mut [u8] {
        let Self::Number(value) = self else {
            unreachable!("NUMBER binding accessor requires a Number binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn date_buffer(&mut self) -> &mut [u8] {
        let Self::Date(value) = self else {
            unreachable!("DATE binding accessor requires an Date binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn short_time_buffer(&mut self) -> &mut [u8] {
        let Self::ShortTime(value) = self else {
            unreachable!("SHORTTIME binding accessor requires a Time binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn timestamp_buffer(&mut self) -> &mut [u8] {
        let Self::Timestamp(value) = self else {
            unreachable!("TIMESTAMP binding accessor requires a Timestamp binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn interval_ym_buffer(&mut self) -> &mut [u8] {
        let Self::IntervalYM(value) = self else {
            unreachable!("YM interval binding accessor requires an IntervalYM binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn interval_ds_buffer(&mut self) -> &mut [u8] {
        let Self::IntervalDS(value) = self else {
            unreachable!("DS interval binding accessor requires an IntervalDS binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub(super) fn text_buffer(&mut self) -> &mut [u8] {
        let Self::Text(value) = self else {
            unreachable!("text binding accessor requires a text buffer")
        };
        // The client treats this as output-only storage and initializes the prefix before it is read.
        unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
    }

    #[inline]
    pub(super) fn binary_buffer(&mut self) -> &mut [u8] {
        let Self::Binary(value) = self else {
            unreachable!("BINARY binding accessor requires a binary buffer")
        };
        // The client treats this as output-only storage and initializes the prefix before it is read.
        unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
    }

    #[inline]
    pub(super) fn lob_buffer(&mut self) -> &mut [u8] {
        let Self::Lob(locator) = self else {
            unreachable!("LOB binding accessor requires a LOB")
        };
        // This is a byte view of the `YacLobLocator *` variable held by the
        // wrapper. `bind_column` passes the view's address as `YacLobLocator **`,
        // as required by the C driver, rather than passing locator contents.
        locator.get_mut().bind_output()
    }
}
