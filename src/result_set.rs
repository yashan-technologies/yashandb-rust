//! Result sets, metadata, rows, and strict column decoding.

use std::mem::MaybeUninit;

use crate::error::Error;
use crate::ffi::{NULL_DATA, YacExtType};
use crate::stmt::Statement;
use crate::types::private::ColumnIndex;
use crate::types::{
    ColumnBinding, ColumnInfo, DataTypeInfo, Date, IntervalDS, IntervalYM, Number, Time, Timestamp, YacNumber,
    YacTimestamp,
};

/// A single-row, streaming result set that exclusively borrows its connection.
///
/// The connection cannot perform another operation until this result set is
/// dropped or [`Self::finish`]ed. Drop releases the native statement; call
/// [`Self::finish`] when an explicit release error must be observed.
pub struct ResultSet<'conn> {
    stmt: Statement<'conn>,
    schema: Vec<ColumnInfo>,
    columns: Vec<ColumnBuffer>,
    eof: bool,
}

impl<'conn> ResultSet<'conn> {
    #[inline]
    pub(crate) fn from_stmt(mut stmt: Statement<'conn>) -> Result<Self, Error> {
        let schema = stmt.schema()?;
        let columns = bind_columns(&mut stmt, &schema)?;

        Ok(Self {
            stmt,
            schema,
            columns,
            eof: false,
        })
    }

    /// Metadata for all result columns.
    #[inline]
    pub fn columns(&self) -> &[ColumnInfo] {
        &self.schema
    }

    /// Fetch the next row, or `Ok(None)` at end of result set.
    ///
    /// Each row borrows this result set's current binding buffers. Borrowed
    /// `&str` and `&[u8]` values returned by [`Row::get`] remain valid only for
    /// as long as the returned row is borrowed. Once end of result set is
    /// reached, subsequent calls return `Ok(None)` without calling the client.
    #[inline]
    pub fn fetch(&mut self) -> Result<Option<Row<'_>>, Error> {
        if self.eof {
            return Ok(None);
        }
        match self.stmt.fetch()? {
            0 => {
                self.eof = true;
                Ok(None)
            }
            1 => Ok(Some(Row {
                schema: &self.schema,
                columns: &self.columns,
            })),
            rows => Err(Error::Internal(format!(
                "client returned {rows} rows for rowset size 1"
            ))),
        }
    }

    /// Release the native statement and propagate a release error.
    ///
    /// This is optional for normal cleanup because [`Drop`] releases the native
    /// statement best-effort. Use this method when the release error matters.
    #[inline]
    pub fn finish(self) -> Result<(), Error> {
        self.stmt.finish()
    }
}

fn bind_columns(stmt: &mut Statement<'_>, schema: &[ColumnInfo]) -> Result<Vec<ColumnBuffer>, Error> {
    let (ratio, nratio) = stmt.charset_ratios()?;
    let mut columns = Vec::with_capacity(schema.len());

    for (index, info) in schema.iter().enumerate() {
        match info.data_type_info {
            DataTypeInfo::Bool => {
                let column = push_column(&mut columns, ColumnBinding::Bool(0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Bool,
                    column.binding.bool_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::TinyInt => {
                let column = push_column(&mut columns, ColumnBinding::TinyInt(0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::TinyInt,
                    column.binding.tiny_int_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::SmallInt => {
                let column = push_column(&mut columns, ColumnBinding::SmallInt(0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::SmallInt,
                    column.binding.small_int_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Integer => {
                let column = push_column(&mut columns, ColumnBinding::Integer(0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Integer,
                    column.binding.integer_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::BigInt => {
                let column = push_column(&mut columns, ColumnBinding::BigInt(0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::BigInt,
                    column.binding.big_int_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Float => {
                let column = push_column(&mut columns, ColumnBinding::Float(0.0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Float,
                    column.binding.float_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Double => {
                let column = push_column(&mut columns, ColumnBinding::Double(0.0));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Double,
                    column.binding.double_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Number { .. } => {
                let column = push_column(&mut columns, ColumnBinding::Number(YacNumber::ZERO));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Number,
                    column.binding.number_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Date => {
                let column = push_column(&mut columns, ColumnBinding::Date(Date::MIN));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Date,
                    column.binding.date_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Time => {
                let column = push_column(&mut columns, ColumnBinding::ShortTime(Time::ZERO));
                stmt.bind_column(
                    index as u16,
                    YacExtType::ShortTime,
                    column.binding.short_time_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Timestamp => {
                let column = push_column(
                    &mut columns,
                    ColumnBinding::Timestamp(YacTimestamp::with_timestamp(Timestamp::MIN)),
                );
                stmt.bind_column(
                    index as u16,
                    YacExtType::Timestamp,
                    column.binding.timestamp_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::IntervalYM => {
                let column = push_column(&mut columns, ColumnBinding::IntervalYM(IntervalYM::ZERO));
                stmt.bind_column(
                    index as u16,
                    YacExtType::YmInterval,
                    column.binding.interval_ym_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::IntervalDS => {
                let column = push_column(&mut columns, ColumnBinding::IntervalDS(IntervalDS::ZERO));
                stmt.bind_column(
                    index as u16,
                    YacExtType::DsInterval,
                    column.binding.interval_ds_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Char { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, ratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Char,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::NChar { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, nratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Char,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::VarChar { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, ratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::VarChar,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::NVarChar { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, nratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::VarChar,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Binary { size } => {
                let column = push_column(&mut columns, ColumnBinding::Binary(variable_buffer(size as usize)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Binary,
                    column.binding.binary_buffer(),
                    &mut column.indicator,
                )?;
            }
            _ => columns.push(ColumnBuffer {
                binding: ColumnBinding::Unsupported,
                indicator: 0,
            }),
        }
    }

    Ok(columns)
}

#[inline]
fn text_buffer(size: u32, ratio: u32) -> Vec<MaybeUninit<u8>> {
    variable_buffer((size as usize).next_multiple_of(4) * ratio as usize)
}

#[inline]
fn variable_buffer(size: usize) -> Vec<MaybeUninit<u8>> {
    vec![MaybeUninit::uninit(); size + 1]
}

#[inline]
fn push_column(columns: &mut Vec<ColumnBuffer>, binding: ColumnBinding) -> &mut ColumnBuffer {
    columns.push(ColumnBuffer { binding, indicator: 0 });
    columns.last_mut().expect("column was just pushed")
}

struct ColumnBuffer {
    binding: ColumnBinding,
    indicator: i32,
}

impl ColumnBuffer {
    #[inline]
    fn as_bool(&self) -> bool {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Bool(value) = &self.binding else {
            unreachable!("BOOL column requires a bool binding")
        };
        *value != 0
    }

    #[inline]
    fn as_i8(&self) -> i8 {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::TinyInt(value) = &self.binding else {
            unreachable!("TINYINT column requires an i8 binding")
        };
        *value
    }

    #[inline]
    fn as_i16(&self) -> i16 {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::SmallInt(value) = &self.binding else {
            unreachable!("SMALLINT column requires an i16 binding")
        };
        *value
    }

    #[inline]
    fn as_i32(&self) -> i32 {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Integer(value) = &self.binding else {
            unreachable!("INTEGER column requires an i32 binding")
        };
        *value
    }

    #[inline]
    fn as_i64(&self) -> i64 {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::BigInt(value) = &self.binding else {
            unreachable!("BIGINT column requires an i64 binding")
        };
        *value
    }

    #[inline]
    fn as_f32(&self) -> f32 {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Float(value) = &self.binding else {
            unreachable!("FLOAT column requires an f32 binding")
        };
        *value
    }

    #[inline]
    fn as_f64(&self) -> f64 {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Double(value) = &self.binding else {
            unreachable!("DOUBLE column requires an f64 binding")
        };
        *value
    }

    #[inline]
    fn as_number(&self) -> Number {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Number(value) = &self.binding else {
            unreachable!("NUMBER column requires a Number binding")
        };
        value.into_number()
    }

    #[inline]
    fn as_text(&self) -> &str {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Text(value) = &self.binding else {
            unreachable!("text column requires a text binding")
        };
        // ConnectionBuilder configures the client environment to return UTF-8 text.
        unsafe { std::str::from_utf8_unchecked(self.variable_bytes(value)) }
    }

    #[inline]
    fn as_string(&self) -> String {
        self.as_text().to_owned()
    }

    #[inline]
    fn as_binary(&self) -> &[u8] {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Binary(value) = &self.binding else {
            unreachable!("BINARY column requires a binary binding")
        };
        self.variable_bytes(value)
    }

    #[inline]
    fn as_vec(&self) -> Vec<u8> {
        self.as_binary().to_vec()
    }

    #[inline]
    fn variable_bytes<'a>(&self, buffer: &'a [MaybeUninit<u8>]) -> &'a [u8] {
        assert!(
            self.indicator >= 0,
            "client returned an invalid non-NULL data indicator"
        );
        let len = self.indicator as usize;
        assert!(
            len <= buffer.len(),
            "client reported a value longer than its binding buffer"
        );
        // The client initializes exactly the indicator-reported prefix during fetch.
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast(), len) }
    }

    #[inline]
    fn as_date(&self) -> Date {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Date(value) = &self.binding else {
            unreachable!("DATE column requires a Date binding")
        };
        *value
    }

    #[inline]
    fn as_time(&self) -> Time {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::ShortTime(value) = &self.binding else {
            unreachable!("SHORTTIME column requires a Time binding")
        };
        *value
    }

    #[inline]
    fn as_timestamp(&self) -> Timestamp {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::Timestamp(value) = &self.binding else {
            unreachable!("TIMESTAMP column requires a Timestamp binding")
        };
        value.timestamp
    }

    #[inline]
    fn as_interval_ym(&self) -> IntervalYM {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::IntervalYM(value) = &self.binding else {
            unreachable!("YM interval column requires an IntervalYM binding")
        };
        *value
    }

    #[inline]
    fn as_interval_ds(&self) -> IntervalDS {
        debug_assert_ne!(self.indicator, NULL_DATA, "cannot read a NULL column value");
        let ColumnBinding::IntervalDS(value) = &self.binding else {
            unreachable!("DS interval column requires an IntervalDS binding")
        };
        *value
    }
}

pub struct Column<'row> {
    index: usize,
    info: &'row ColumnInfo,
    buffer: &'row ColumnBuffer,
}

impl<'row> Column<'row> {
    #[inline]
    fn is_null(&self) -> bool {
        self.buffer.indicator == NULL_DATA
    }

    #[inline]
    fn read<T>(
        &self,
        expected: &'static str,
        matches_type: impl FnOnce(DataTypeInfo) -> bool,
        read: impl FnOnce(&'row ColumnBuffer) -> T,
    ) -> Result<T, Error> {
        if self.is_null() {
            return Err(Error::NullValue { index: self.index });
        }

        self.read_non_null(expected, matches_type, read)
    }

    #[inline]
    fn read_optional<T>(
        &self,
        expected: &'static str,
        matches_type: impl FnOnce(DataTypeInfo) -> bool,
        read: impl FnOnce(&'row ColumnBuffer) -> T,
    ) -> Result<Option<T>, Error> {
        if self.is_null() {
            Ok(None)
        } else {
            Ok(Some(self.read_non_null(expected, matches_type, read)?))
        }
    }

    fn read_non_null<T>(
        &self,
        expected: &'static str,
        matches_type: impl FnOnce(DataTypeInfo) -> bool,
        read: impl FnOnce(&'row ColumnBuffer) -> T,
    ) -> Result<T, Error> {
        if !matches_type(self.info.data_type_info) {
            return Err(Error::ColumnTypeMismatch {
                index: self.index,
                expected,
                actual: self.info.data_type_info.data_type(),
            });
        }

        Ok(read(self.buffer))
    }
}

/// A borrowed view of the current result row.
pub struct Row<'row> {
    schema: &'row [ColumnInfo],
    columns: &'row [ColumnBuffer],
}

impl<'row> Row<'row> {
    /// Metadata for all columns in this row.
    #[inline]
    pub fn columns(&self) -> &'row [ColumnInfo] {
        self.schema
    }

    /// Number of columns in this row.
    #[inline]
    pub fn column_count(&self) -> usize {
        self.schema.len()
    }

    /// Read one column from the row buffer as a Rust value.
    ///
    /// Pass a `usize` zero-based column index or the exact database-reported
    /// `&str` name. When a name matches multiple columns, the first matching
    /// column is read.
    /// The requested Rust type must exactly match the database column type; this
    /// method does not perform numeric, text, or other implicit conversions.
    ///
    /// The SQL-to-Rust mapping is:
    ///
    /// | SQL type | Rust target type |
    /// | --- | --- |
    /// | `BOOLEAN` | `bool` |
    /// | `TINYINT` | `i8` |
    /// | `SMALLINT` | `i16` |
    /// | `INTEGER` | `i32` |
    /// | `BIGINT` | `i64` |
    /// | `FLOAT` | `f32` |
    /// | `DOUBLE` | `f64` |
    /// | `NUMBER` | [`Number`] |
    /// | `DATE` | [`Date`] |
    /// | `TIME` | [`Time`] |
    /// | `TIMESTAMP` | [`Timestamp`] |
    /// | `INTERVAL YEAR TO MONTH` | [`IntervalYM`] |
    /// | `INTERVAL DAY TO SECOND` | [`IntervalDS`] |
    /// | `CHAR`, `VARCHAR`, `NCHAR`, `NVARCHAR` | `String` or `&str` |
    /// | `BINARY` | `Vec<u8>` or `&[u8]` |
    ///
    /// `TIMESTAMP WITH LOCAL TIME ZONE`, `TIMESTAMP WITH TIME ZONE`, and
    /// unrecognized SQL types are not supported and return
    /// [`Error::UnsupportedColumnType`]. Wrap any mapped type in `Option`, such
    /// as `Option<String>`, to read a database `NULL`; requesting a
    /// non-optional type for `NULL` returns [`Error::NullValue`]. Borrowed text
    /// and binary values are valid only for as long as the row is borrowed.
    ///
    /// Returns an error when the index or name is absent, the driver cannot
    /// represent the column type, the target type does not match, or text data
    /// is not valid UTF-8. Inspect [`Self::columns`] when selecting the
    /// appropriate target type. Text is decoded to UTF-8 by the client driver.
    ///
    /// `TimestampLtz`, `TimestampTz`, and [`DataTypeInfo::Other`] are available
    /// through metadata but return [`Error::UnsupportedColumnType`] only when
    /// that column is read. The supported target types are exactly the table
    /// entries and their `Option` wrappers; custom target and index types cannot
    /// be supplied.
    ///
    /// ```no_run
    /// use yashandb::{Connection, Error};
    ///
    /// # fn read_user(conn: &mut Connection) -> Result<(), Error> {
    /// let mut rows = conn.query("select id, name from users")?;
    /// let row = rows.fetch()?.ok_or(Error::RowNotFound)?;
    /// let id: i32 = row.get(0)?;
    /// let name: Option<String> = row.get("NAME")?;
    /// # let _ = (id, name);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn get<T: private::FromColumn<'row>>(&self, index: impl ColumnIndex) -> Result<T, Error> {
        let column = self.column(index.index(self.schema)?)?;
        T::from_column(&column)
    }

    fn column(&self, index: usize) -> Result<Column<'row>, Error> {
        debug_assert_eq!(self.schema.len(), self.columns.len());

        let info = self.schema.get(index).ok_or(Error::ColumnIndexOutOfBounds {
            index,
            column_count: self.schema.len(),
        })?;
        let column = &self.columns[index];

        if matches!(column.binding, ColumnBinding::Unsupported) {
            return Err(Error::UnsupportedColumnType {
                index,
                name: info.name.clone(),
                data_type: info.data_type_info.data_type(),
            });
        }

        Ok(Column {
            index,
            info,
            buffer: column,
        })
    }
}

pub(super) mod private {
    use super::*;

    pub trait FromColumn<'row>: Sized {
        fn from_column(column: &Column<'row>) -> Result<Self, Error>;
    }

    macro_rules! impl_from_column {
        ($t:ty, $name:literal, $kind:pat, $read:ident) => {
            impl<'row> FromColumn<'row> for $t {
                #[inline]
                fn from_column(c: &Column<'row>) -> Result<Self, Error> {
                    c.read($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
                }
            }
            impl<'row> FromColumn<'row> for Option<$t> {
                #[inline]
                fn from_column(c: &Column<'row>) -> Result<Self, Error> {
                    c.read_optional($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
                }
            }
        };
        (borrow $t:ty, $name:literal, $kind:pat, $read:ident) => {
            impl<'row> FromColumn<'row> for &'row $t {
                #[inline]
                fn from_column(c: &Column<'row>) -> Result<Self, Error> {
                    c.read($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
                }
            }
            impl<'row> FromColumn<'row> for Option<&'row $t> {
                #[inline]
                fn from_column(c: &Column<'row>) -> Result<Self, Error> {
                    c.read_optional($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
                }
            }
        };
    }

    impl_from_column!(bool, "bool", DataTypeInfo::Bool, as_bool);

    impl_from_column!(i8, "i8", DataTypeInfo::TinyInt, as_i8);
    impl_from_column!(i16, "i16", DataTypeInfo::SmallInt, as_i16);
    impl_from_column!(i32, "i32", DataTypeInfo::Integer, as_i32);
    impl_from_column!(i64, "i64", DataTypeInfo::BigInt, as_i64);
    impl_from_column!(f32, "f32", DataTypeInfo::Float, as_f32);
    impl_from_column!(f64, "f64", DataTypeInfo::Double, as_f64);
    impl_from_column!(Number, "Number", DataTypeInfo::Number { .. }, as_number);

    impl_from_column!(Date, "Date", DataTypeInfo::Date, as_date);
    impl_from_column!(Time, "Time", DataTypeInfo::Time, as_time);
    impl_from_column!(Timestamp, "Timestamp", DataTypeInfo::Timestamp, as_timestamp);
    impl_from_column!(IntervalYM, "IntervalYM", DataTypeInfo::IntervalYM, as_interval_ym);
    impl_from_column!(IntervalDS, "IntervalDS", DataTypeInfo::IntervalDS, as_interval_ds);

    impl_from_column!(
        borrow str,
        "&str",
        DataTypeInfo::Char { .. }
            | DataTypeInfo::VarChar { .. }
            | DataTypeInfo::NChar { .. }
            | DataTypeInfo::NVarChar { .. },
        as_text
    );
    impl_from_column!(
        String,
        "String",
        DataTypeInfo::Char { .. }
            | DataTypeInfo::VarChar { .. }
            | DataTypeInfo::NChar { .. }
            | DataTypeInfo::NVarChar { .. },
        as_string
    );
    impl_from_column!(borrow[u8], "&[u8]", DataTypeInfo::Binary { .. }, as_binary);
    impl_from_column!(Vec<u8>, "Vec<u8>", DataTypeInfo::Binary { .. }, as_vec);
}
