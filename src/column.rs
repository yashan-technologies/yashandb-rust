//! Result column metadata.

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
    /// [`crate::Error::UnsupportedColumnType`] when read from a row.
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

#[cfg(test)]
mod tests {
    use super::{DataType, DataTypeInfo};

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
}
