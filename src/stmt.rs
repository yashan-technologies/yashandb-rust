//! Safe statement operations tied to a connection.

use std::mem::MaybeUninit;
use std::slice;

use crate::conn::Connection;
use crate::error::Error;
use crate::ffi::{StmtHandle, YacExtType, YacType};
use crate::result_set::ResultSet;
use crate::types::{ColumnInfo, DataTypeInfo};

const COLUMN_NAME_BUFFER_SIZE: usize = 256;

/// A statement allocated from and exclusively borrowing a connection.
pub(crate) struct Statement<'conn> {
    conn: &'conn mut Connection,
    stmt: StmtHandle,
}

impl<'conn> Statement<'conn> {
    #[inline]
    pub fn new(conn: &'conn mut Connection) -> Result<Self, Error> {
        let stmt = conn.alloc_stmt()?;
        Ok(Self { conn, stmt })
    }

    #[inline]
    pub fn execute(&mut self, sql: &str) -> Result<ExecResult, Error> {
        let lib = self.conn.lib();
        lib.direct_execute(&mut self.stmt, sql)?;
        Ok(ExecResult {
            rows_affected: lib.get_stmt_rows_affected(&self.stmt)?,
        })
    }

    #[inline]
    pub fn query(mut self, sql: &str) -> Result<ResultSet<'conn>, Error> {
        self.conn.lib().direct_execute(&mut self.stmt, sql)?;
        ResultSet::from_stmt(self)
    }

    #[inline]
    pub fn schema(&self) -> Result<Vec<ColumnInfo>, Error> {
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
    pub fn charset_ratios(&self) -> Result<(u32, u32), Error> {
        self.conn.charset_ratios()
    }

    #[inline]
    pub fn bind_column(
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
    pub fn fetch(&mut self) -> Result<u32, Error> {
        self.conn.lib().fetch(&mut self.stmt)
    }

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
