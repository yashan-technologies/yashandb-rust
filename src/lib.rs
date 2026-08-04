//! An official Rust driver for [YashanDB](https://www.yashandb.com), providing a
//! synchronous, blocking connection to a YashanDB instance.
//!
//! The driver loads the YashanDB client library (`yascli`) at runtime and
//! connects through it. The library is found automatically on first use, or can
//! be loaded explicitly with [`load_library`] / [`load_library_with_path`].
//!
//! # Example
//!
//! ```no_run
//! use yashandb::Connection;
//!
//! # fn main() -> Result<(), yashandb::Error> {
//! let conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
//! # let _ = conn;
//! # Ok(())
//! # }
//! ```
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

pub use conn::Connection;
pub use error::Error;
pub use library::{load_library, load_library_with_path};
