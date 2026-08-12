//! An official Rust driver for [YashanDB](https://www.yashandb.com), providing a
//! synchronous, blocking connection to a YashanDB instance.
//!
//! The driver loads the YashanDB client library (`yascli`) at runtime and
//! connects through it. The library is found automatically on first use, or can
//! be loaded explicitly with [`load_library`] / [`load_library_with_path`].
//! It supports non-parameterized SQL through [`Connection::execute`] and
//! [`Connection::query`]. Query results are streamed one row at a time through
//! [`ResultSet::fetch`].
//!
//! # Example
//!
//! ```no_run
//! use yashandb::Connection;
//!
//! # fn main() -> Result<(), yashandb::Error> {
//! let mut conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
//! let answer = conn.query_one_map("select 42 from dual", |row| row.get::<i32>(0))?;
//! assert_eq!(answer, 42);
//! # Ok(())
//! # }
//! ```
//!
//! # Queries
//!
//! [`Connection::query`] exclusively borrows its connection until the returned
//! [`ResultSet`] is dropped or finished. Use [`ResultSet::finish`] when an
//! explicit native-statement release error is needed; otherwise normal Rust
//! drop cleanup releases the statement. Result values support the types listed
//! by [`Row::get`], including `Option<T>` for database `NULL` values.
//!
//! # Client library version
//!
//! The driver targets the YashanDB client library **23.4.1.100** and later as
//! its baseline: connection attributes the baseline supports are treated as
//! infallible (the underlying call `.expect()`s internally), so a client
//! library older than that baseline may not provide every attribute and could
//! panic. Only attributes that legitimately fail at runtime (e.g. setting the
//! transaction isolation mid-transaction, or the baseline-unsupported login
//! timeout) return a `Result`.
//!
//! # Thread safety
//!
//! [`Connection`] is `Send` but not `Sync`: it can be moved to another thread,
//! but must not be shared across threads.
//!
//! # Feature flags
//!
//! This crate has no feature flags.

#![warn(missing_docs)]

mod conn;
mod error;
mod ffi;
mod library;
mod load;
mod result_set;
mod stmt;
mod types;

pub use conn::{Connection, ConnectionBuilder, TransactionIsolation};
pub use error::Error;
pub use library::{load_library, load_library_with_path};
pub use result_set::{ResultSet, Row};
pub use stmt::ExecResult;
pub use types::{ColumnInfo, DataType, DataTypeInfo, Date, IntervalDS, IntervalYM, Number, Time, Timestamp};
