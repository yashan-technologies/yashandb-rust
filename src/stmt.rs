//! Safe statement operations tied to a connection.

use std::mem::MaybeUninit;
use std::slice;

use crate::column::{ColumnInfo, DataTypeInfo};
use crate::conn::Connection;
use crate::error::Error;
use crate::ffi::{StmtHandle, YacExtType, YacType};
use crate::param::{BindParam, NamedBindParam};
use crate::result_set::ResultSet;

const COLUMN_NAME_BUFFER_SIZE: usize = 256;

/// A statement allocated from and exclusively borrowing a connection.
pub struct Statement<'conn> {
    conn: &'conn mut Connection,
    stmt: StmtHandle,
    state: StatementState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatementState {
    /// The statement can be prepared or executed.
    Ready,
    /// A query has an active result set that must be drained before reuse.
    Active,
    /// Result cleanup failed, so the native statement must not be reused.
    Poisoned,
}

impl<'conn> Statement<'conn> {
    #[inline]
    pub(crate) fn new(conn: &'conn mut Connection) -> Result<Self, Error> {
        let stmt = conn.alloc_stmt()?;
        Ok(Self {
            conn,
            stmt,
            state: StatementState::Ready,
        })
    }

    #[inline]
    pub(crate) fn direct_execute(&mut self, sql: &str) -> Result<ExecResult, Error> {
        let lib = self.conn.lib();
        lib.direct_execute(&mut self.stmt, sql)?;
        Ok(ExecResult {
            rows_affected: lib.get_stmt_rows_affected(&self.stmt)?,
        })
    }

    #[inline]
    pub(crate) fn direct_query(mut self, sql: &str) -> Result<ResultSet<'conn, 'conn>, Error> {
        self.conn.lib().direct_execute(&mut self.stmt, sql)?;
        ResultSet::from_stmt(self)
    }

    #[inline]
    pub(crate) fn prepare(&mut self, sql: &str) -> Result<(), Error> {
        self.conn.lib().prepare(&mut self.stmt, sql)
    }

    /// Execute this prepared statement with positional parameters.
    ///
    /// `params` is an array, slice, or `Vec` of [`BindParam`] values. Use
    /// [`crate::input`] for input values, [`crate::output`] for output targets, and
    /// [`crate::in_out`] for input/output targets.
    #[inline]
    pub fn execute<'a>(&mut self, mut params: impl AsMut<[BindParam<'a>]>) -> Result<ExecResult, Error> {
        self.execute_params(params.as_mut(), false)
    }

    /// Execute this prepared statement with named parameters.
    ///
    /// Each item must be created with [`crate::named`]. The name is passed to
    /// the client without a SQL placeholder prefix, so `:value` in SQL is
    /// paired with the name `value`.
    #[inline]
    pub fn execute_named<'name, 'param>(
        &mut self,
        mut params: impl AsMut<[NamedBindParam<'name, 'param>]>,
    ) -> Result<ExecResult, Error> {
        self.execute_named_params(params.as_mut(), false)
    }

    /// Execute this prepared query with positional parameters.
    ///
    /// `params` is an array, slice, or `Vec` of [`BindParam`] values.
    ///
    /// The returned result set retains the statement's mutable borrow until it
    /// is finished or dropped, so the same statement cannot be executed while
    /// its rows are being streamed. See the crate-level
    /// [interface usage](crate#interface-usage) documentation for complete
    /// examples of positional, named, input, output, and input/output binds.
    #[inline]
    pub fn query<'stmt, 'param>(
        &'stmt mut self,
        mut params: impl AsMut<[BindParam<'param>]>,
    ) -> Result<ResultSet<'conn, 'stmt>, Error>
    where
        'conn: 'stmt,
    {
        self.execute_params(params.as_mut(), true)?;
        ResultSet::from_borrowed(self)
    }

    /// Execute this prepared query with named parameters.
    ///
    /// Names are passed to the client without a SQL placeholder prefix. For
    /// example, use `named(c"value", input(1_i64))` for `:value`.
    #[inline]
    pub fn query_named<'stmt, 'name, 'param>(
        &'stmt mut self,
        mut params: impl AsMut<[NamedBindParam<'name, 'param>]>,
    ) -> Result<ResultSet<'conn, 'stmt>, Error>
    where
        'conn: 'stmt,
    {
        self.execute_named_params(params.as_mut(), true)?;
        ResultSet::from_borrowed(self)
    }

    #[inline]
    pub(crate) fn query_owned<'stmt, 'param>(
        mut self,
        params: &mut [BindParam<'param>],
    ) -> Result<ResultSet<'conn, 'stmt>, Error>
    where
        'conn: 'stmt,
    {
        self.execute_params(params, true)?;
        ResultSet::from_stmt(self)
    }

    #[inline]
    pub(crate) fn query_named_owned<'stmt, 'name, 'param>(
        mut self,
        params: &mut [NamedBindParam<'name, 'param>],
    ) -> Result<ResultSet<'conn, 'conn>, Error>
    where
        'conn: 'stmt,
    {
        self.execute_named_params(params, true)?;
        ResultSet::from_stmt(self)
    }

    fn execute_params(&mut self, params: &mut [BindParam<'_>], query: bool) -> Result<ExecResult, Error> {
        self.ensure_ready()?;

        let expected = self.conn.lib().num_params(&self.stmt)?;
        if params.len() != expected as usize {
            return Err(Error::InvalidArgument(format!(
                "prepared statement expects {expected} parameters, received {}",
                params.len()
            )));
        }

        for (id, param) in (1..=expected).zip(params.iter_mut()) {
            let binding = param.binding();
            self.conn.lib().bind_parameter(&mut self.stmt, id, binding)?;
        }

        self.conn.lib().execute(&mut self.stmt)?;

        // The native query is active even if output decoding fails. Mark it
        // before decoding so the statement cannot be reused in that state.
        if query {
            self.state = StatementState::Active;
        }

        for param in params {
            if let Err(error) = param.complete() {
                if query {
                    let _ = self.finish_result();
                }
                return Err(error);
            }
        }

        if query {
            Ok(ExecResult { rows_affected: 0 })
        } else {
            Ok(ExecResult {
                rows_affected: self.conn.lib().get_stmt_rows_affected(&self.stmt)?,
            })
        }
    }

    fn execute_named_params(
        &mut self,
        params: &mut [NamedBindParam<'_, '_>],
        query: bool,
    ) -> Result<ExecResult, Error> {
        self.ensure_ready()?;

        for index in 0..params.len() {
            if params[..index].iter().any(|other| other.name() == params[index].name()) {
                return Err(Error::InvalidArgument(format!(
                    "duplicate named parameter {:?}",
                    params[index].name()
                )));
            }
        }

        let expected = self.conn.lib().num_params(&self.stmt)?;
        if params.len() != expected as usize {
            return Err(Error::InvalidArgument(format!(
                "prepared statement expects {expected} parameters, received {}",
                params.len()
            )));
        }

        for param in params.iter_mut() {
            let (name, param) = param.parts_mut();
            let binding = param.binding();
            self.conn.lib().bind_parameter_by_name(&mut self.stmt, name, binding)?;
        }

        self.conn.lib().execute(&mut self.stmt)?;

        if query {
            self.state = StatementState::Active;
        }

        for param in params {
            if let Err(error) = param.param_mut().complete() {
                if query {
                    let _ = self.finish_result();
                }
                return Err(error);
            }
        }

        if query {
            Ok(ExecResult { rows_affected: 0 })
        } else {
            Ok(ExecResult {
                rows_affected: self.conn.lib().get_stmt_rows_affected(&self.stmt)?,
            })
        }
    }

    #[inline]
    fn ensure_ready(&self) -> Result<(), Error> {
        match self.state {
            StatementState::Ready => Ok(()),
            StatementState::Active => Err(Error::Internal("prepared statement has an active result set".into())),
            StatementState::Poisoned => Err(Error::Internal(
                "prepared statement cannot be reused after result cleanup failed".into(),
            )),
        }
    }

    pub(crate) fn finish_result(&mut self) -> Result<(), Error> {
        if self.state != StatementState::Active {
            return Ok(());
        }

        // Drain the remaining rows so the client releases the active result set
        // and the statement can be reused.
        loop {
            match self.fetch() {
                Ok(0) => {
                    self.state = StatementState::Ready;
                    return Ok(());
                }
                Ok(1) => {}
                Ok(rows) => {
                    self.state = StatementState::Poisoned;
                    return Err(Error::Internal(format!(
                        "client returned {rows} rows for rowset size 1"
                    )));
                }
                Err(error) => {
                    self.state = StatementState::Poisoned;
                    return Err(error);
                }
            }
        }
    }

    #[inline]
    pub(crate) fn schema(&self) -> Result<Vec<ColumnInfo>, Error> {
        let count = self.result_column_count()?;

        let mut schema = Vec::with_capacity(count as usize);
        for id in 0..count {
            schema.push(self.column_info(id)?);
        }

        Ok(schema)
    }

    fn column_info(&self, id: u16) -> Result<ColumnInfo, Error> {
        let name = self.column_name(id)?;

        let data_type_info = match self.column_type(id)? {
            YacType::Bool => DataTypeInfo::Bool,
            YacType::TinyInt => DataTypeInfo::TinyInt,
            YacType::SmallInt => DataTypeInfo::SmallInt,
            YacType::Integer => DataTypeInfo::Integer,
            YacType::BigInt => DataTypeInfo::BigInt,
            YacType::Float => DataTypeInfo::Float,
            YacType::Double => DataTypeInfo::Double,
            YacType::Number => DataTypeInfo::Number {
                precision: self.column_precision(id)?,
                scale: self.column_scale(id)?,
            },
            YacType::Date => DataTypeInfo::Date,
            YacType::ShortTime => DataTypeInfo::Time,
            YacType::Timestamp => DataTypeInfo::Timestamp,
            YacType::TimestampLtz => DataTypeInfo::TimestampLtz,
            YacType::TimestampTz => DataTypeInfo::TimestampTz,
            YacType::YmInterval => DataTypeInfo::IntervalYM,
            YacType::DsInterval => DataTypeInfo::IntervalDS,
            YacType::Char => DataTypeInfo::Char {
                size: self.column_size(id)?,
                char_size: self.column_char_size(id)?,
            },
            YacType::NChar => DataTypeInfo::NChar {
                size: self.column_size(id)?,
                char_size: self.column_char_size(id)?,
            },
            YacType::VarChar => DataTypeInfo::VarChar {
                size: self.column_size(id)?,
                char_size: self.column_char_size(id)?,
            },
            YacType::NVarChar => DataTypeInfo::NVarChar {
                size: self.column_size(id)?,
                char_size: self.column_char_size(id)?,
            },
            YacType::Binary => DataTypeInfo::Binary {
                size: self.column_size(id)?,
            },
            value => DataTypeInfo::Other(value as u8),
        };

        let nullable = self.column_nullable(id)?;

        Ok(ColumnInfo {
            name,
            data_type_info,
            nullable,
        })
    }

    #[inline]
    fn result_column_count(&self) -> Result<u16, Error> {
        self.conn.lib().get_num_result_cols(&self.stmt)
    }

    #[inline]
    fn column_name(&self, index: u16) -> Result<String, Error> {
        let mut name_bytes: [MaybeUninit<u8>; COLUMN_NAME_BUFFER_SIZE] = unsafe { MaybeUninit::uninit().assume_init() };
        // The client initializes the returned name bytes and reports their length.
        let name_buffer = unsafe { slice::from_raw_parts_mut(name_bytes.as_mut_ptr().cast::<u8>(), name_bytes.len()) };
        let name_len = self.conn.lib().get_stmt_col_name(&self.stmt, index, name_buffer)?;
        assert!(
            name_len as usize <= name_buffer.len(),
            "client reported a column name longer than its output buffer"
        );
        let name = unsafe { slice::from_raw_parts(name_bytes.as_ptr().cast::<u8>(), name_len as usize).to_vec() };
        String::from_utf8(name).map_err(|_| Error::InvalidEncoding { context: "column name" })
    }

    #[inline]
    fn column_type(&self, index: u16) -> Result<YacType, Error> {
        self.conn.lib().get_stmt_col_type(&self.stmt, index)
    }

    #[inline]
    fn column_size(&self, index: u16) -> Result<u32, Error> {
        self.conn.lib().get_stmt_col_size(&self.stmt, index)
    }

    #[inline]
    fn column_char_size(&self, index: u16) -> Result<u32, Error> {
        self.conn.lib().get_stmt_col_char_size(&self.stmt, index)
    }

    #[inline]
    fn column_precision(&self, index: u16) -> Result<u8, Error> {
        self.conn.lib().get_stmt_col_precision(&self.stmt, index)
    }

    #[inline]
    fn column_scale(&self, index: u16) -> Result<i8, Error> {
        self.conn.lib().get_stmt_col_scale(&self.stmt, index)
    }

    #[inline]
    fn column_nullable(&self, index: u16) -> Result<bool, Error> {
        self.conn.lib().get_stmt_col_nullable(&self.stmt, index)
    }

    #[inline]
    pub(crate) fn charset_ratios(&self) -> Result<(u32, u32), Error> {
        self.conn.charset_ratios()
    }

    #[inline]
    pub(crate) fn bind_column(
        &mut self,
        index: u16,
        ext_type: YacExtType,
        buffer: &mut [u8],
        indicator: &mut i32,
    ) -> Result<(), Error> {
        self.conn
            .lib()
            .bind_column(&mut self.stmt, index, ext_type, buffer, indicator)
    }

    #[inline]
    pub(crate) fn fetch(&mut self) -> Result<u32, Error> {
        self.conn.lib().fetch(&mut self.stmt)
    }

    /// Release this prepared statement and propagate a native release error.
    ///
    /// Normal drop cleanup releases the statement best-effort. This method is
    /// useful when an explicit release error must be observed.
    #[inline]
    pub fn finish(mut self) -> Result<(), Error> {
        let result = self.conn.lib().free_stmt(&mut self.stmt);
        std::mem::forget(self);
        result
    }
}

impl Drop for Statement<'_> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.conn.lib().free_stmt(&mut self.stmt);
    }
}

/// The result of executing a statement without a result set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecResult {
    rows_affected: u64,
}

impl ExecResult {
    /// Number of rows affected by the statement.
    #[inline]
    pub const fn rows_affected(&self) -> u64 {
        self.rows_affected
    }
}
