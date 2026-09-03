//! An official Rust driver for [YashanDB](https://www.yashandb.com), providing a
//! synchronous, blocking connection to a YashanDB instance.
//!
//! The driver loads the YashanDB client library (`yascli`) at runtime and
//! connects through it. The library is found automatically on first use, or can
//! be loaded explicitly with [`load_library`] / [`load_library_with_path`].
//! It supports non-parameterized SQL through [`Connection::execute`] and
//! [`Connection::query`], and parameterized SQL through the convenience methods
//! [`Connection::execute_with`] / [`Connection::query_with`] or a reusable
//! [`Statement`] from [`Connection::prepare`]. Query results are streamed one
//! row at a time through [`ResultSet::fetch`]. Transactions use manual commit
//! by default. Use [`Connection::commit`] and [`Connection::rollback`] in that
//! mode, or enable auto-commit and use [`Connection::transaction`] for a scoped
//! transaction.
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
//! [`Connection::query`] shares its connection until the returned [`ResultSet`]
//! is dropped or finished. Other statements on the same connection may execute
//! while rows are streamed. Use [`ResultSet::finish`] when an explicit
//! native-statement release error is needed; otherwise normal Rust drop cleanup
//! releases the statement. Result values support the types listed by
//! [`Row::get`], including `Option<T>` for database `NULL` values.
//!
//! # Transactions
//!
//! New connections use manual commit by default. Finish directly managed work
//! with [`Connection::commit`] or discard it with [`Connection::rollback`];
//! neither changes the connection's auto-commit setting:
//!
//! ```no_run
//! # use yashandb::{Connection, Error};
//! # fn example(conn: &mut Connection) -> Result<(), Error> {
//! conn.execute("insert into audit_log(message) values ('created')")?;
//! conn.commit()?;
//! # Ok(())
//! # }
//! ```
//!
//! For a scoped transaction on a connection configured with auto-commit, use
//! [`Connection::transaction`]. It temporarily disables auto-commit, and
//! restores it after commit, rollback, or drop. Dropping an unfinished guard
//! attempts to roll back; call [`Transaction::rollback`] explicitly when its
//! error must be observed:
//!
//! ```no_run
//! # use yashandb::{Connection, Error};
//! # fn example(conn: &mut Connection) -> Result<(), Error> {
//! conn.set_auto_commit(true);
//! let mut tx = conn.transaction();
//! tx.execute("update accounts set balance = balance - 10 where id = 1")?;
//! tx.execute("update accounts set balance = balance + 10 where id = 2")?;
//! tx.commit()?;
//! # Ok(())
//! # }
//! ```
//!
//! In manual-commit mode, a scoped transaction guard owns the current
//! connection transaction, including work pending before it was created. It
//! does not start a separate server transaction or support nesting. A result
//! set or statement created from a guard must be finished or dropped before the
//! guard can be committed or rolled back.
//!
//! # Interface usage
//!
//! Use [`Connection::execute`] for SQL without parameters that does not return
//! rows, and [`Connection::query`] for a streaming result set:
//!
//! ```no_run
//! # use yashandb::{Connection, Error};
//! # fn example(conn: &mut Connection) -> Result<(), Error> {
//! conn.execute("insert into users(id, name) values (1, 'Alice')")?;
//! let mut rows = conn.query("select id, name from users")?;
//! while let Some(row) = rows.fetch()? {
//!     let id: i32 = row.get(0)?;
//!     let name: Option<String> = row.get("NAME")?;
//!     let _ = (id, name);
//! }
//! rows.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! Use [`Connection::execute_with`] and [`Connection::query_with`] for
//! parameterized SQL that is executed once. Construct values with [`input`],
//! [`output`], and [`in_out`]:
//!
//! ```no_run
//! # use yashandb::{Connection, Error, input};
//! # fn example(conn: &mut Connection, id: i64, name: &str) -> Result<(), Error> {
//! conn.execute_with(
//!     "insert into users(id, name) values (?, ?)",
//!     [input(id), input(name)],
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! For repeated execution, use [`Connection::prepare`] and call
//! [`Statement::execute`] or [`Statement::query`] on the returned statement:
//!
//! ```no_run
//! # use yashandb::{Connection, Error, input};
//! # fn example(conn: &mut Connection, ids: &[i64]) -> Result<(), Error> {
//! let mut stmt = conn.prepare("select id from users where id = ?")?;
//! for &id in ids {
//!     let mut rows = stmt.query([input(id)])?;
//!     while let Some(row) = rows.fetch()? {
//!         let _: i64 = row.get(0)?;
//!     }
//!     rows.finish()?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Named parameters use [`Connection::execute_named_with`],
//! [`Connection::query_named_with`], [`Statement::execute_named`], or
//! [`Statement::query_named`]. Pass a NUL-terminated [`std::ffi::CStr`] or
//! [`std::ffi::CString`] to [`named`]. The name is passed without the SQL
//! placeholder prefix, so `value` corresponds to `:value`:
//!
//! ```no_run
//! # use std::ffi::CString;
//! # use yashandb::{Connection, Error, input, named};
//! # fn example(conn: &mut Connection, id: i64) -> Result<(), Error> {
//! let name = CString::new("id").map_err(|_| Error::InvalidArgument("invalid parameter name".into()))?;
//! conn.execute_named_with(
//!     "begin process_user(:id); end;",
//!     [named(name, input(id))],
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! Input `Option<T>` values represent SQL `NULL`. Output targets are passed by
//! mutable reference and are updated after execution; variable-size targets
//! use their existing capacity. For nullable variable-size output, pass a
//! minimum buffer capacity such as `output((&mut value, 128))`; an existing
//! larger allocation may be reused.
//! [`output`] only receives the database value, while [`in_out`] also sends the
//! target's initial value:
//!
//! If execution returns an error, output and input/output targets may already
//! contain data written by the client. Their updates are not atomic.
//!
//! Query results are read with [`Row::get`]. The supported SQL-to-Rust
//! mappings are:
//!
//! | SQL type | Rust type |
//! | --- | --- |
//! | `BOOL` | `bool` / `Option<bool>` |
//! | `TINYINT` | `i8` / `Option<i8>` |
//! | `SMALLINT` | `i16` / `Option<i16>` |
//! | `INTEGER` | `i32` / `Option<i32>` |
//! | `BIGINT` | `i64` / `Option<i64>` |
//! | `FLOAT` | `f32` / `Option<f32>` |
//! | `DOUBLE` | `f64` / `Option<f64>` |
//! | `NUMBER` | [`Number`] / `Option<`[`Number`]`>` |
//! | `DATE` | [`Date`] / `Option<`[`Date`]`>` |
//! | `SHORTTIME` | [`Time`] / `Option<`[`Time`]`>` |
//! | `TIMESTAMP` | [`Timestamp`] / `Option<`[`Timestamp`]`>` |
//! | `INTERVAL YEAR TO MONTH` | [`IntervalYM`] / `Option<`[`IntervalYM`]`>` |
//! | `INTERVAL DAY TO SECOND` | [`IntervalDS`] / `Option<`[`IntervalDS`]`>` |
//! | `CHAR`, `NCHAR`, `VARCHAR`, `NVARCHAR` | `String`, `&str`, or their `Option<T>` forms |
//! | `BINARY` | `Vec<u8>`, `&[u8]`, or their `Option<T>` forms |
//! | `JSON` | [`YasonBuf`], [`Yason`], or their `Option<T>` forms |
//! | `BLOB` | [`Blob`] / `Option<`[`Blob`]`>` |
//! | `CLOB`, `NCLOB` | [`Clob`] / `Option<`[`Clob`]`>` |
//!
//! JSON query values can be read as [`Yason`], [`YasonBuf`], or their optional
//! forms. A borrowed [`Yason`] is valid only while the current row is borrowed;
//! use [`YasonBuf`] when the value must outlive the row. Reading JSON SQL `NULL`
//! into a non-optional target returns [`Error::NullValue`], while an optional
//! target receives `None`.
//!
//! LOB query values are locators, not eagerly materialized contents. Extract a
//! [`Blob`] or [`Clob`] with [`Row::get`], use its positioned I/O methods, and
//! drop or [`Blob::finish`] / [`Clob::finish`] it before the owning connection
//! is dropped. [`Blob`] offsets are one-based bytes; [`Clob`] offsets and lengths
//! are one-based characters. [`Clob`] is the Rust representation for both CLOB
//! and NCLOB columns. Reading a LOB into a `Vec<u8>` or `String` is explicit via
//! [`Blob::read_to_end`] or [`Clob::read_to_string`]. Extracting a LOB transfers
//! its locator from the row binding, so the same LOB column can be extracted
//! only once for that row. Later fetches rebind transferred LOB columns, and
//! transferred locators remain usable after [`ResultSet::finish`].
//!
//! Temporary LOBs are created with [`Connection::temporary_blob`] or
//! [`Connection::temporary_clob`]. They are server-side temporary values and
//! can be passed as input parameters with [`input`]. Pure OUT LOB parameters
//! use `output(&mut Option<Blob>)` / `output(&mut Option<Clob>)`, or a locator
//! allocated by [`Connection::output_blob`] / [`Connection::output_clob`] for
//! non-nullable output. LOB `in_out` parameters are not supported; use [`input`]
//! or [`output`] for LOB parameters.
//!
//! ```no_run
//! # use yashandb::{Blob, Connection, Error, input};
//! # fn example(conn: &mut Connection) -> Result<(), Error> {
//! let mut blob = conn.temporary_blob()?;
//! blob.append(b"payload")?;
//! conn.execute_with("insert into files(data) values (?)", [input(&blob)])?;
//! blob.finish()?;
//!
//! let mut result = conn.query("select data from files")?;
//! let row = result.fetch()?.expect("one row");
//! let mut fetched: Blob<'_> = row.get(0)?;
//! let mut bytes = Vec::new();
//! fetched.read_to_end(&mut bytes)?;
//! drop(fetched);
//! result.finish()?;
//! # Ok(())
//! # }
//! ```
//! For parameter binding, the direction is reversed: the Rust type determines
//! the YashanDB/YACLI type:
//!
//! | Rust type | YashanDB/YACLI type |
//! | --- | --- |
//! | `bool` / `Option<bool>` | `BOOL` |
//! | `i8` / `Option<i8>` | `TINYINT` |
//! | `i16` / `Option<i16>` | `SMALLINT` |
//! | `i32` / `Option<i32>` | `INTEGER` |
//! | `i64` / `Option<i64>` | `BIGINT` |
//! | `f32` / `Option<f32>` | `FLOAT` |
//! | `f64` / `Option<f64>` | `DOUBLE` |
//! | [`Number`] / `Option<`[`Number`]`>` | `NUMBER` |
//! | [`Date`] / `Option<`[`Date`]`>` | `DATE` |
//! | [`Time`] / `Option<`[`Time`]`>` | `SHORTTIME` |
//! | [`Timestamp`] / `Option<`[`Timestamp`]`>` | `TIMESTAMP` |
//! | [`IntervalYM`] / `Option<`[`IntervalYM`]`>` | `INTERVAL YEAR TO MONTH` |
//! | [`IntervalDS`] / `Option<`[`IntervalDS`]`>` | `INTERVAL DAY TO SECOND` |
//! | `&str` / `String` and their `Option<T>` forms | `VARCHAR` |
//! | `&[u8]` / `Vec<u8>` and their `Option<T>` forms | `BINARY` |
//! | [`YasonBuf`] / `&`[`Yason`] and their `Option<T>` forms | `JSON` input |
//! | `&mut `[`YasonBuf`] / `&mut Option<`[`YasonBuf`]`>` | `JSON` output |
//! | `&`[`Blob`] / `Option<&`[`Blob`]`>` | `BLOB` input |
//! | `&mut `[`Blob`] / `&mut Option<`[`Blob`]`>` | `BLOB` output |
//! | `&`[`Clob`] / `Option<&`[`Clob`]`>` | `CLOB` input |
//! | `&mut `[`Clob`] / `&mut Option<`[`Clob`]`>` | `CLOB` output |
//!
//! The corresponding `Option<T>` input and output forms keep the same database
//! type and add SQL `NULL` handling. LOB `in_out` parameters are not supported;
//! use [`input`] or [`output`] for LOB parameters.
//! JSON `in_out` parameters are not supported; use separate [`input`] and
//! [`output`] parameters instead.
//! Create JSON values with [`YasonBuf::parse`]. Use [`YasonBuf`] for owned
//! values, [`Yason`] for borrowed input, and `Option` forms when SQL `NULL` is
//! allowed.
//!
//! ```no_run
//! # use yashandb::{Connection, Error, in_out, named, output};
//! # fn example(conn: &mut Connection) -> Result<(), Error> {
//! let mut value = 7_i64;
//! let mut stmt = conn.prepare("begin :value := :value + 1; end;")?;
//! stmt.execute_named([named(c"value", in_out(&mut value))])?;
//! stmt.finish()?;
//!
//! let mut text = String::with_capacity(128);
//! let mut stmt = conn.prepare("begin :text := 'hello'; end;")?;
//! stmt.execute_named([named(c"text", output(&mut text))])?;
//! # Ok(())
//! # }
//! ```
//!
//! A result set borrows its connection, or its prepared statement, until it is
//! finished or dropped. Finish the result set before reusing a prepared
//! statement.
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

mod column;
mod conn;
mod error;
mod ffi;
mod library;
mod load;
mod lob;
mod param;
mod result_set;
mod stmt;
mod transaction;
mod types;

pub use column::{ColumnInfo, DataType, DataTypeInfo};
pub use conn::{Connection, ConnectionBuilder, TransactionIsolation};
pub use error::Error;
pub use library::{load_library, load_library_with_path};
pub use lob::{Blob, Clob};
pub use param::{BindParam, NamedBindParam, in_out, input, named, output};
pub use result_set::{ResultSet, Row};
pub use stmt::{ExecResult, Statement};
pub use transaction::Transaction;
pub use types::{Date, IntervalDS, IntervalYM, Number, Time, Timestamp, Yason, YasonBuf};
