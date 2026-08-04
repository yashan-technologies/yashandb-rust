//! Error types for the YashanDB driver.

use std::fmt;

/// Errors returned by the YashanDB driver.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Library file not found, failed to load, or a required symbol missing.
    ClientLibrary(String),
    /// A caller-supplied argument is invalid (e.g. a connection parameter too long).
    InvalidArgument(String),
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ClientLibrary(msg) => write!(f, "client library error: {msg}"),
            Error::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            Error::Database {
                code,
                message,
                line,
                column,
            } => {
                write!(f, "database error [{code}] at {line}:{column}: {message}")
            }
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
    fn implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<Error>();
    }
}
