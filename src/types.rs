//! Public database type and column metadata definitions.

use std::mem::MaybeUninit;

use crate::error::Error;

/// Day-to-second interval value.
pub use sqldatetime::IntervalDS;
/// Year-to-month interval value.
pub use sqldatetime::IntervalYM;
/// Calendar date value.
pub use sqldatetime::OracleDate as Date;
/// Time-of-day value.
pub use sqldatetime::Time;
/// Timestamp without a time zone.
pub use sqldatetime::Timestamp;

/// Database NUMBER value.
///
/// This representation supports up to 38 significant decimal digits.
pub use decimal_rs::Decimal as Number;

/// The database type reported for a result column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DataType {
    /// Boolean value.
    Bool,
    /// Signed 8-bit integer.
    TinyInt,
    /// Signed 16-bit integer.
    SmallInt,
    /// Signed 32-bit integer.
    Integer,
    /// Signed 64-bit integer.
    BigInt,
    /// Single-precision floating-point number.
    Float,
    /// Double-precision floating-point number.
    Double,
    /// Decimal number.
    Number,
    /// Calendar date.
    Date,
    /// Time of day without a date.
    Time,
    /// Timestamp without a time zone.
    Timestamp,
    /// Timestamp in the session time zone, retained as metadata only.
    TimestampLtz,
    /// Timestamp with a time zone, retained as metadata only.
    TimestampTz,
    /// Year-to-month interval.
    IntervalYM,
    /// Day-to-second interval.
    IntervalDS,
    /// Fixed-width character data.
    Char,
    /// Fixed-width national character data.
    NChar,
    /// Variable-width character data.
    VarChar,
    /// Variable-width national character data.
    NVarChar,
    /// Binary data.
    Binary,
    /// A database type not recognized by this driver, retained as metadata only.
    Other(u8),
}

/// Type-specific metadata reported for a result column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DataTypeInfo {
    /// Boolean value.
    Bool,
    /// Signed 8-bit integer.
    TinyInt,
    /// Signed 16-bit integer.
    SmallInt,
    /// Signed 32-bit integer.
    Integer,
    /// Signed 64-bit integer.
    BigInt,
    /// Single-precision floating-point number.
    Float,
    /// Double-precision floating-point number.
    Double,
    /// Decimal number.
    Number {
        /// Maximum number of significant decimal digits.
        precision: u8,
        /// Number of digits to the right of the decimal point.
        scale: i8,
    },
    /// Calendar date.
    Date,
    /// Time of day without a date.
    Time,
    /// Timestamp without a time zone.
    Timestamp,
    /// Timestamp in the session time zone, retained as metadata only.
    TimestampLtz,
    /// Timestamp with a time zone, retained as metadata only.
    TimestampTz,
    /// Year-to-month interval.
    IntervalYM,
    /// Day-to-second interval.
    IntervalDS,
    /// Fixed-width character data.
    Char {
        /// Maximum byte length.
        size: u32,
        /// Maximum character length.
        char_size: u32,
    },
    /// Fixed-width national character data.
    NChar {
        /// Maximum byte length.
        size: u32,
        /// Maximum character length.
        char_size: u32,
    },
    /// Variable-width character data.
    VarChar {
        /// Maximum byte length.
        size: u32,
        /// Maximum character length.
        char_size: u32,
    },
    /// Variable-width national character data.
    NVarChar {
        /// Maximum byte length.
        size: u32,
        /// Maximum character length.
        char_size: u32,
    },
    /// Binary data.
    Binary {
        /// Maximum byte length.
        size: u32,
    },
    /// A database type not recognized by this driver, retained as metadata only.
    Other(u8),
}

impl DataTypeInfo {
    /// General database type for this type information.
    #[inline]
    pub const fn data_type(&self) -> DataType {
        match self {
            Self::Bool => DataType::Bool,
            Self::TinyInt => DataType::TinyInt,
            Self::SmallInt => DataType::SmallInt,
            Self::Integer => DataType::Integer,
            Self::BigInt => DataType::BigInt,
            Self::Float => DataType::Float,
            Self::Double => DataType::Double,
            Self::Number { .. } => DataType::Number,
            Self::Date => DataType::Date,
            Self::Time => DataType::Time,
            Self::Timestamp => DataType::Timestamp,
            Self::TimestampLtz => DataType::TimestampLtz,
            Self::TimestampTz => DataType::TimestampTz,
            Self::IntervalYM => DataType::IntervalYM,
            Self::IntervalDS => DataType::IntervalDS,
            Self::Char { .. } => DataType::Char,
            Self::NChar { .. } => DataType::NChar,
            Self::VarChar { .. } => DataType::VarChar,
            Self::NVarChar { .. } => DataType::NVarChar,
            Self::Binary { .. } => DataType::Binary,
            Self::Other(value) => DataType::Other(*value),
        }
    }
}

/// Immutable metadata for one result column.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnInfo {
    pub(crate) name: String,
    pub(crate) data_type_info: DataTypeInfo,
    pub(crate) nullable: bool,
}

impl ColumnInfo {
    /// Column name as returned by the client.
    ///
    /// Name lookups through [`crate::Row::get`] use an exact, case-sensitive
    /// match against this value.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Type-specific metadata for this column.
    ///
    /// Character `size` values are maximum byte lengths; `char_size` values are
    /// maximum character lengths. Metadata-only types return
    /// [`Error::UnsupportedColumnType`] when read from a row.
    #[inline]
    pub const fn data_type_info(&self) -> DataTypeInfo {
        self.data_type_info
    }

    /// Whether the database permits NULL.
    #[inline]
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}

pub(crate) mod private {
    use super::*;

    /// A column position or database-reported column name.
    pub trait ColumnIndex {
        /// Resolve this index against result column metadata.
        fn index(&self, columns: &[ColumnInfo]) -> Result<usize, Error>;
    }

    impl ColumnIndex for usize {
        #[inline]
        fn index(&self, columns: &[ColumnInfo]) -> Result<usize, Error> {
            if *self < columns.len() {
                Ok(*self)
            } else {
                Err(Error::ColumnIndexOutOfBounds {
                    index: *self,
                    column_count: columns.len(),
                })
            }
        }
    }

    impl ColumnIndex for &str {
        #[inline]
        fn index(&self, columns: &[ColumnInfo]) -> Result<usize, Error> {
            columns
                .iter()
                .position(|info| info.name == *self)
                .ok_or_else(|| Error::ColumnNotFound {
                    name: (*self).to_owned(),
                })
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub(crate) struct YacNumber {
    value: u128,
    sign: i8,
    _unused: u8,
    exp: i16,
}

impl YacNumber {
    pub const ZERO: YacNumber = YacNumber {
        value: 0,
        sign: 0,
        _unused: 0,
        exp: 0,
    };

    #[inline]
    pub const fn into_number(self) -> Number {
        // YacNumber's exponent has the opposite sign from Number's scale.
        // YacExtType::Number fills YacNumber with a valid database NUMBER value.
        unsafe { Number::from_parts_unchecked(self.value, -self.exp, self.sign != 0) }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub(crate) struct YacTimestamp {
    pub timestamp: Timestamp,
    pub bias: i16, // minutes
    _unused: i16,
}

impl YacTimestamp {
    #[inline]
    pub const fn with_timestamp(timestamp: Timestamp) -> Self {
        YacTimestamp {
            timestamp,
            bias: 0,
            _unused: 0,
        }
    }
}

/// Storage bound to a result column for one fetched row.
///
/// Each variant owns storage with the Rust representation matching its database
/// type. Text and binary values retain uninitialized storage because the client
/// writes their variable-length contents during fetch.
pub(crate) enum ColumnBinding {
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
}

impl ColumnBinding {
    #[inline]
    fn scalar_buffer<T>(value: &mut T) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(std::ptr::from_mut(value).cast(), size_of_val(value)) }
    }

    #[inline]
    pub fn bool_buffer(&mut self) -> &mut [u8] {
        let Self::Bool(value) = self else {
            unreachable!("BOOL binding accessor requires a bool binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn tiny_int_buffer(&mut self) -> &mut [u8] {
        let Self::TinyInt(value) = self else {
            unreachable!("TINYINT binding accessor requires an i8 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn small_int_buffer(&mut self) -> &mut [u8] {
        let Self::SmallInt(value) = self else {
            unreachable!("SMALLINT binding accessor requires an i16 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn integer_buffer(&mut self) -> &mut [u8] {
        let Self::Integer(value) = self else {
            unreachable!("INTEGER binding accessor requires an i32 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn big_int_buffer(&mut self) -> &mut [u8] {
        let Self::BigInt(value) = self else {
            unreachable!("BIGINT binding accessor requires an i64 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn float_buffer(&mut self) -> &mut [u8] {
        let Self::Float(value) = self else {
            unreachable!("FLOAT binding accessor requires an f32 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn double_buffer(&mut self) -> &mut [u8] {
        let Self::Double(value) = self else {
            unreachable!("DOUBLE binding accessor requires an f64 binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn number_buffer(&mut self) -> &mut [u8] {
        let Self::Number(value) = self else {
            unreachable!("NUMBER binding accessor requires a Number binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn date_buffer(&mut self) -> &mut [u8] {
        let Self::Date(value) = self else {
            unreachable!("DATE binding accessor requires an Date binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn short_time_buffer(&mut self) -> &mut [u8] {
        let Self::ShortTime(value) = self else {
            unreachable!("SHORTTIME binding accessor requires a Time binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn timestamp_buffer(&mut self) -> &mut [u8] {
        let Self::Timestamp(value) = self else {
            unreachable!("TIMESTAMP binding accessor requires a Timestamp binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn interval_ym_buffer(&mut self) -> &mut [u8] {
        let Self::IntervalYM(value) = self else {
            unreachable!("YM interval binding accessor requires an IntervalYM binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn interval_ds_buffer(&mut self) -> &mut [u8] {
        let Self::IntervalDS(value) = self else {
            unreachable!("DS interval binding accessor requires an IntervalDS binding")
        };
        Self::scalar_buffer(value)
    }

    #[inline]
    pub fn text_buffer(&mut self) -> &mut [u8] {
        let Self::Text(value) = self else {
            unreachable!("text binding accessor requires a text buffer")
        };
        // The client treats this as output-only storage and initializes the
        // indicator-reported prefix before the buffer is read.
        unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
    }

    #[inline]
    pub fn binary_buffer(&mut self) -> &mut [u8] {
        let Self::Binary(value) = self else {
            unreachable!("BINARY binding accessor requires a binary buffer")
        };
        // The client treats this as output-only storage and initializes the
        // indicator-reported prefix before the buffer is read.
        unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataType, DataTypeInfo, YacNumber};

    #[test]
    fn data_type_info_maps_to_data_type() {
        let cases = [
            (DataTypeInfo::Bool, DataType::Bool),
            (DataTypeInfo::TinyInt, DataType::TinyInt),
            (DataTypeInfo::SmallInt, DataType::SmallInt),
            (DataTypeInfo::Integer, DataType::Integer),
            (DataTypeInfo::BigInt, DataType::BigInt),
            (DataTypeInfo::Float, DataType::Float),
            (DataTypeInfo::Double, DataType::Double),
            (
                DataTypeInfo::Number {
                    precision: 10,
                    scale: 2,
                },
                DataType::Number,
            ),
            (DataTypeInfo::Date, DataType::Date),
            (DataTypeInfo::Time, DataType::Time),
            (DataTypeInfo::Timestamp, DataType::Timestamp),
            (DataTypeInfo::TimestampLtz, DataType::TimestampLtz),
            (DataTypeInfo::TimestampTz, DataType::TimestampTz),
            (DataTypeInfo::IntervalYM, DataType::IntervalYM),
            (DataTypeInfo::IntervalDS, DataType::IntervalDS),
            (DataTypeInfo::Char { size: 8, char_size: 8 }, DataType::Char),
            (DataTypeInfo::NChar { size: 8, char_size: 8 }, DataType::NChar),
            (DataTypeInfo::VarChar { size: 8, char_size: 8 }, DataType::VarChar),
            (DataTypeInfo::NVarChar { size: 8, char_size: 8 }, DataType::NVarChar),
            (DataTypeInfo::Binary { size: 8 }, DataType::Binary),
            (DataTypeInfo::Other(255), DataType::Other(255)),
        ];

        for (info, expected) in cases {
            assert_eq!(info.data_type(), expected);
        }
    }

    #[test]
    fn yac_number_layout() {
        assert_eq!(size_of::<YacNumber>(), 20);
        assert_eq!(align_of::<YacNumber>(), 4);
    }
}
