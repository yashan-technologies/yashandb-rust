//! Error types for the YashanDB driver.

use std::fmt;

use crate::types::DataType;

/// Errors returned by the YashanDB driver.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Library file not found, failed to load, or a required symbol missing.
    ClientLibrary(String),
    /// A caller-supplied argument is invalid (e.g. a connection parameter too long).
    InvalidArgument(String),
    /// The driver encountered an internal inconsistency or a broken client contract.
    Internal(String),
    /// C driver returned `YAC_ERROR`; populated via `yacGetDiagRec`.
    Database {
        /// Error code.
        code: i32,
        /// Error message (null-terminated C string, up to 8192 bytes).
        message: String,
        /// Line number where the error occurred.
        line: i32,
        /// Column number where the error occurred.
        column: i32,
    },
    /// A requested column index is outside the result set.
    ColumnIndexOutOfBounds {
        /// The requested zero-based column index.
        index: usize,
        /// The number of columns in the result set.
        column_count: usize,
    },
    /// A requested column name does not exist in the result set.
    ColumnNotFound {
        /// The requested column name.
        name: String,
    },
    /// A NULL database value was read through a non-optional target type.
    NullValue {
        /// The zero-based column index containing NULL.
        index: usize,
    },
    /// The requested Rust type does not match the database column type.
    ColumnTypeMismatch {
        /// The zero-based column index with the mismatch.
        index: usize,
        /// The Rust type expected by the conversion.
        expected: &'static str,
        /// The database type reported for the column.
        actual: DataType,
    },
    /// Bytes returned by the YashanDB C client library are not valid UTF-8 in
    /// the indicated context.
    InvalidEncoding {
        /// The value context in which decoding failed.
        context: &'static str,
    },
    /// A result column cannot be represented by this version of the driver.
    UnsupportedColumnType {
        /// The zero-based column index with the unsupported type.
        index: usize,
        /// The column name reported by the database.
        name: String,
        /// The database type reported for the column.
        data_type: DataType,
    },
    /// A one-row query returned no rows.
    RowNotFound,
    /// A one-row query returned more than one row.
    TooManyRows,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ClientLibrary(msg) => write!(f, "client library error: {msg}"),
            Error::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            Error::Internal(msg) => write!(f, "internal driver error: {msg}"),
            Error::Database {
                code,
                message,
                line,
                column,
            } => {
                write!(f, "database error [{code}] at {line}:{column}: {message}")
            }
            Error::ColumnIndexOutOfBounds { index, column_count } => {
                write!(f, "column index {index} is out of bounds for {column_count} columns")
            }
            Error::ColumnNotFound { name } => write!(f, "column named {name:?} was not found"),
            Error::NullValue { index } => write!(f, "column {index} is NULL"),
            Error::ColumnTypeMismatch {
                index,
                expected,
                actual,
            } => write!(f, "column {index} has type {actual:?}, not {expected}"),
            Error::InvalidEncoding { context } => write!(f, "invalid UTF-8 in {context}"),
            Error::UnsupportedColumnType { index, name, data_type } => {
                write!(f, "unsupported column type {data_type:?} for column {index} ({name})")
            }
            Error::RowNotFound => f.write_str("query returned no rows"),
            Error::TooManyRows => f.write_str("query returned more than one row"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_client_library() {
        let e = Error::ClientLibrary("boom".into());
        assert_eq!(e.to_string(), "client library error: boom");
    }

    #[test]
    fn display_invalid_argument() {
        let e = Error::InvalidArgument("bad".into());
        assert_eq!(e.to_string(), "invalid argument: bad");
    }

    #[test]
    fn display_internal() {
        let e = Error::Internal("broken invariant".into());
        assert_eq!(e.to_string(), "internal driver error: broken invariant");
    }

    #[test]
    fn display_database() {
        let e = Error::Database {
            code: 42,
            message: "oops".into(),
            line: 3,
            column: 7,
        };
        assert_eq!(e.to_string(), "database error [42] at 3:7: oops");
    }

    #[test]
    fn display_column_and_query_errors() {
        assert_eq!(
            Error::ColumnIndexOutOfBounds {
                index: 3,
                column_count: 2
            }
            .to_string(),
            "column index 3 is out of bounds for 2 columns"
        );
        assert_eq!(
            Error::ColumnNotFound { name: "missing".into() }.to_string(),
            "column named \"missing\" was not found"
        );
        assert_eq!(Error::NullValue { index: 1 }.to_string(), "column 1 is NULL");
        assert_eq!(
            Error::InvalidEncoding { context: "column name" }.to_string(),
            "invalid UTF-8 in column name"
        );
        assert_eq!(Error::RowNotFound.to_string(), "query returned no rows");
        assert_eq!(Error::TooManyRows.to_string(), "query returned more than one row");
        assert_eq!(
            Error::ColumnTypeMismatch {
                index: 0,
                expected: "i32",
                actual: DataType::VarChar
            }
            .to_string(),
            "column 0 has type VarChar, not i32"
        );
        assert_eq!(
            Error::UnsupportedColumnType {
                index: 0,
                name: "payload".into(),
                data_type: DataType::Other(99)
            }
            .to_string(),
            "unsupported column type Other(99) for column 0 (payload)"
        );
    }

    #[test]
    fn implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<Error>();
    }
}
