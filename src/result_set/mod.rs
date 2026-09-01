//! Result sets, metadata, rows, and strict column decoding.

mod binding;
mod convert;
mod index;

use std::cell::RefCell;
use std::mem::MaybeUninit;

use crate::column::{ColumnInfo, DataTypeInfo};
use crate::error::Error;
use crate::ffi::{NULL_DATA, YacExtType};
use crate::lob::{Blob, Clob, Lob};
use crate::stmt::Statement;
use crate::types::{Date, IntervalDS, IntervalYM, Number, Time, Timestamp, YacNumber, YacTimestamp};

use self::binding::ColumnBinding;

/// A single-row, streaming result set that shares its connection.
///
/// Other statements on the same connection may execute while this result set is
/// active. For a connection query, cleanup releases its temporary native
/// statement. For a prepared statement query, cleanup ends the current result
/// stream and makes the statement reusable. Call [`Self::finish`] when an
/// explicit cleanup error must be observed.
pub struct ResultSet<'conn, 'stmt> {
    statement: StatementAccess<'conn, 'stmt>,
    schema: Vec<ColumnInfo>,
    columns: Vec<ColumnBuffer<'conn>>,
    has_rebindable_columns: bool,
    pending_rebinds: RefCell<Vec<usize>>,
    eof: bool,
}

enum StatementAccess<'conn, 'stmt> {
    Owned(Statement<'conn>),
    Borrowed(&'stmt mut Statement<'conn>),
    None,
}

impl<'conn, 'stmt> StatementAccess<'conn, 'stmt> {
    #[inline]
    fn as_mut(&mut self) -> &mut Statement<'conn> {
        match self {
            Self::Owned(stmt) => stmt,
            Self::Borrowed(stmt) => stmt,
            Self::None => unreachable!("result set statement has already been taken"),
        }
    }

    #[inline]
    fn take(&mut self) -> Self {
        std::mem::replace(self, Self::None)
    }
}

impl<'conn> ResultSet<'conn, 'conn> {
    #[inline]
    pub(crate) fn from_stmt(mut stmt: Statement<'conn>) -> Result<Self, Error> {
        let schema = stmt.schema()?;
        let (columns, has_rebindable_columns) = bind_columns(&mut stmt, &schema)?;

        Ok(Self {
            statement: StatementAccess::Owned(stmt),
            schema,
            columns,
            has_rebindable_columns,
            pending_rebinds: RefCell::new(Vec::new()),
            eof: false,
        })
    }
}

impl<'conn, 'stmt> ResultSet<'conn, 'stmt>
where
    'conn: 'stmt,
{
    #[inline]
    pub(crate) fn from_borrowed(stmt: &'stmt mut Statement<'conn>) -> Result<ResultSet<'conn, 'stmt>, Error> {
        let schema = match stmt.schema() {
            Ok(schema) => schema,
            Err(error) => {
                let _ = stmt.finish_result();
                return Err(error);
            }
        };
        let (columns, has_rebindable_columns) = match bind_columns(stmt, &schema) {
            Ok(result) => result,
            Err(error) => {
                let _ = stmt.finish_result();
                return Err(error);
            }
        };

        Ok(ResultSet {
            statement: StatementAccess::Borrowed(stmt),
            schema,
            columns,
            has_rebindable_columns,
            pending_rebinds: RefCell::new(Vec::new()),
            eof: false,
        })
    }

    #[inline]
    fn statement(&mut self) -> &mut Statement<'conn> {
        self.statement.as_mut()
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
    pub fn fetch(&mut self) -> Result<Option<Row<'conn, '_>>, Error> {
        if self.eof {
            return Ok(None);
        }
        if self.has_rebindable_columns && !self.pending_rebinds.borrow().is_empty() {
            self.rebind_transferred_columns()?;
        }
        match self.statement().fetch()? {
            0 => {
                self.eof = true;
                Ok(None)
            }
            1 => Ok(Some(Row {
                schema: &self.schema,
                columns: &self.columns,
                pending_rebinds: &self.pending_rebinds,
            })),
            rows => Err(Error::Internal(format!(
                "client returned {rows} rows for rowset size 1"
            ))),
        }
    }

    fn rebind_transferred_columns(&mut self) -> Result<(), Error> {
        let (statement, columns, pending_rebinds) = (&mut self.statement, &mut self.columns, &mut self.pending_rebinds);
        let statement = statement.as_mut();
        let pending_rebinds = pending_rebinds.get_mut();
        let mut failure = None;
        for (position, index) in pending_rebinds.iter().copied().enumerate() {
            let info = self.schema[index].data_type_info;
            match info {
                DataTypeInfo::Blob | DataTypeInfo::Clob | DataTypeInfo::Nclob => {
                    if let Err(error) = rebind_lob_column(statement, &mut columns[index], index, info) {
                        failure = Some((position, error));
                        break;
                    }
                }
                _ => unreachable!("column type does not support rebinding"),
            }
        }
        if let Some((position, error)) = failure {
            // Remove completed entries in place and retain this and later
            // entries for a retry without replacing the Vec.
            pending_rebinds.drain(..position);
            return Err(error);
        }
        pending_rebinds.clear();
        Ok(())
    }

    /// Finish the result stream and propagate a cleanup error.
    ///
    /// This is optional for normal cleanup because [`Drop`] performs best-effort
    /// cleanup. For a query created by [`crate::Statement`], this leaves the
    /// prepared statement ready for reuse.
    #[inline]
    pub fn finish(mut self) -> Result<(), Error> {
        let statement = self.statement.take();
        match statement {
            StatementAccess::Owned(stmt) => stmt.finish(),
            StatementAccess::Borrowed(stmt) => stmt.finish_result(),
            StatementAccess::None => Ok(()),
        }
    }
}

impl Drop for ResultSet<'_, '_> {
    #[inline]
    fn drop(&mut self) {
        if let StatementAccess::Borrowed(stmt) = &mut self.statement {
            let _ = stmt.finish_result();
        }
    }
}

fn bind_columns<'conn>(
    stmt: &mut Statement<'conn>,
    schema: &[ColumnInfo],
) -> Result<(Vec<ColumnBuffer<'conn>>, bool), Error> {
    let (ratio, nratio) = stmt.charset_ratios()?;
    let mut columns = Vec::with_capacity(schema.len());
    let mut has_rebindable_columns = false;

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
                    YacExtType::Char2,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::NChar { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, nratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Char2,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::VarChar { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, ratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Varchar2,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::NVarChar { size, .. } => {
                let column = push_column(&mut columns, ColumnBinding::Text(text_buffer(size, nratio)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Varchar2,
                    column.binding.text_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Binary { size } => {
                let column = push_column(&mut columns, ColumnBinding::Binary(variable_buffer(size as usize)));
                stmt.bind_column(
                    index as u16,
                    YacExtType::Binary2,
                    column.binding.binary_buffer(),
                    &mut column.indicator,
                )?;
            }
            DataTypeInfo::Blob | DataTypeInfo::Clob | DataTypeInfo::Nclob => {
                has_rebindable_columns = true;
                let locator = Lob::new(stmt.connection())?;
                let column = push_column(&mut columns, ColumnBinding::Lob(RefCell::new(locator)));
                stmt.bind_column(
                    index as u16,
                    if matches!(info.data_type_info, DataTypeInfo::Blob) {
                        YacExtType::Blob
                    } else {
                        // Bind NCLOB as CLOB so YACLI converts UTF-16 to UTF-8.
                        YacExtType::Clob
                    },
                    column.binding.lob_buffer(),
                    &mut column.indicator,
                )?;
            }
            _ => columns.push(ColumnBuffer {
                binding: ColumnBinding::Unsupported,
                indicator: 0,
            }),
        }
    }

    Ok((columns, has_rebindable_columns))
}

fn rebind_lob_column<'conn>(
    statement: &mut Statement<'conn>,
    column: &mut ColumnBuffer<'conn>,
    index: usize,
    info: DataTypeInfo,
) -> Result<(), Error> {
    debug_assert!(matches!(&column.binding, ColumnBinding::Lob(lob) if !lob.borrow().is_live()));
    let locator = Lob::new(statement.connection())?;
    column.binding = ColumnBinding::Lob(RefCell::new(locator));
    column.indicator = 0;
    let ext_type = if matches!(info, DataTypeInfo::Blob) {
        YacExtType::Blob
    } else {
        // Keep the rebind external type consistent with the initial NCLOB bind.
        YacExtType::Clob
    };
    statement.bind_column(
        index as u16,
        ext_type,
        column.binding.lob_buffer(),
        &mut column.indicator,
    )
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
fn push_column<'conn, 'column>(
    columns: &'column mut Vec<ColumnBuffer<'conn>>,
    binding: ColumnBinding<'conn>,
) -> &'column mut ColumnBuffer<'conn> {
    columns.push(ColumnBuffer { binding, indicator: 0 });
    columns.last_mut().expect("column was just pushed")
}

struct ColumnBuffer<'conn> {
    binding: ColumnBinding<'conn>,
    indicator: i32,
}

impl<'conn> ColumnBuffer<'conn> {
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
    fn as_blob(&self, index: usize, pending_rebinds: &RefCell<Vec<usize>>) -> Result<Blob<'conn>, Error> {
        self.take_lob(index, pending_rebinds).map(Blob::from_lob)
    }

    #[inline]
    fn as_clob(
        &self,
        index: usize,
        pending_rebinds: &RefCell<Vec<usize>>,
        data_type_info: DataTypeInfo,
    ) -> Result<Clob<'conn>, Error> {
        self.take_lob(index, pending_rebinds).map(|lob| {
            let lob_type = match data_type_info {
                DataTypeInfo::Nclob => crate::ffi::YacTempLobType::NClob,
                _ => crate::ffi::YacTempLobType::Clob,
            };
            Clob::from_lob(lob, lob_type)
        })
    }

    #[inline]
    fn take_lob(&self, index: usize, pending_rebinds: &RefCell<Vec<usize>>) -> Result<Lob<'conn>, Error> {
        let ColumnBinding::Lob(lob) = &self.binding else {
            unreachable!("LOB column requires a LOB binding")
        };
        let lob = lob.borrow_mut().take().ok_or(Error::ColumnValueTransferred { index })?;
        pending_rebinds.borrow_mut().push(index);
        Ok(lob)
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

pub struct Column<'conn: 'row, 'row> {
    info: &'row ColumnInfo,
    buffer: &'row ColumnBuffer<'conn>,
    index: usize,
    pending_rebinds: &'row RefCell<Vec<usize>>,
}

impl<'conn: 'row, 'row> Column<'conn, 'row> {
    #[inline]
    fn is_null(&self) -> bool {
        self.buffer.indicator == NULL_DATA
    }

    #[inline]
    fn read<T>(
        &self,
        expected: &'static str,
        matches_type: impl FnOnce(DataTypeInfo) -> bool,
        read: impl FnOnce(&'row ColumnBuffer<'conn>) -> T,
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
        read: impl FnOnce(&'row ColumnBuffer<'conn>) -> T,
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
        read: impl FnOnce(&'row ColumnBuffer<'conn>) -> T,
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
pub struct Row<'conn: 'row, 'row> {
    schema: &'row [ColumnInfo],
    columns: &'row [ColumnBuffer<'conn>],
    pending_rebinds: &'row RefCell<Vec<usize>>,
}

impl<'conn: 'row, 'row> Row<'conn, 'row> {
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
    /// | `BLOB` | [`crate::Blob`] |
    /// | `CLOB`, `NCLOB` | [`crate::Clob`] |
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
    /// be supplied. LOB targets own a locator transferred from the row, so a
    /// given LOB column can be read only once per row.
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
    pub fn get<T: convert::FromColumn<'conn, 'row>>(&self, index: impl index::ColumnIndex) -> Result<T, Error>
    where
        'conn: 'row,
    {
        let column = self.column(index.index(self.schema)?)?;
        T::from_column(&column)
    }

    fn column(&self, index: usize) -> Result<Column<'conn, 'row>, Error> {
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
            info,
            buffer: column,
            index,
            pending_rebinds: self.pending_rebinds,
        })
    }
}
