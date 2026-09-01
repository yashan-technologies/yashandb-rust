//! Blocking connection to a YashanDB instance.

use std::cell::UnsafeCell;

use crate::error::Error;
use crate::ffi;
use crate::library;
use crate::param::{BindParam, NamedBindParam};
use crate::result_set::ResultSet;
use crate::stmt::{ExecResult, Statement};
use crate::transaction::Transaction;

/// A synchronous blocking connection to a YashanDB instance.
///
/// `Send` so the connection can be moved to a worker thread; `!Sync` because the
/// underlying C handles must not be accessed concurrently from multiple threads.
pub struct Connection {
    lib: &'static ffi::YacLib,
    handle: UnsafeCell<ConnectionHandle>,
}

struct ConnectionHandle {
    env: ffi::EnvHandle,
    dbc: ffi::DbcHandle,
}

impl Connection {
    /// Connect to a YashanDB instance.
    ///
    /// Returns a `Connection` only on success.
    ///
    /// # URL format
    ///
    /// The `url` argument supports the following formats:
    ///
    /// - Single address: `host:port`
    /// - Multiple addresses: `serverType:host:port,host:port,host:port`
    ///   (addresses separated by `,`)
    /// - Multiple address groups:
    ///   `serverType:host:port,host:port;host:port,host:port`
    ///   (groups separated by `;`, addresses within a group separated by `,`)
    ///
    /// `host` may be an IPv4 address, IPv6 address, or domain name. `port` is
    /// the server listening port, defaulting to `1688`.
    ///
    /// `serverType` is optional and may be one of:
    /// - `primary` (default): connect addresses in order, keep the first
    ///   connection to a primary node
    /// - `standby`: connect addresses in order, keep the first connection to a
    ///   standby node
    /// - `loadBalance`: shuffle addresses, pick the node with the fewest
    ///   sessions
    /// - `primaryLoadBalance`: shuffle addresses, pick the primary node with
    ///   the fewest sessions
    /// - `standbyLoadBalance`: shuffle addresses, pick the standby node with
    ///   the fewest sessions
    #[inline]
    pub fn connect(url: &str, username: &str, password: &str) -> Result<Self, Error> {
        Connection::builder().connect(url, username, password)
    }

    /// Start building a connection with configurable attributes.
    ///
    /// See [`ConnectionBuilder`] for the attributes that can be set before
    /// connecting.
    #[inline]
    pub const fn builder() -> ConnectionBuilder {
        ConnectionBuilder::new()
    }

    #[inline]
    pub(crate) fn lib(&self) -> &'static ffi::YacLib {
        self.lib
    }

    #[inline]
    fn with_connection_handle<T>(&self, operation: impl FnOnce(&ffi::YacLib, &mut ConnectionHandle) -> T) -> T {
        // SAFETY: Connection is !Sync, and each FFI call receives exclusive access
        // to the DBC handle only for the duration of this callback.
        unsafe { operation(self.lib, &mut *self.handle.get()) }
    }

    #[inline]
    pub(crate) fn with_handle<T>(&self, operation: impl FnOnce(&ffi::YacLib, &mut ffi::DbcHandle) -> T) -> T {
        self.with_connection_handle(|lib, handle| operation(lib, &mut handle.dbc))
    }

    /// Create an empty temporary binary LOB owned by this connection.
    #[inline]
    pub fn temporary_blob(&self) -> Result<crate::Blob<'_>, Error> {
        crate::Blob::temporary(self)
    }

    /// Create an empty temporary character LOB owned by this connection.
    #[inline]
    pub fn temporary_clob(&self) -> Result<crate::Clob<'_>, Error> {
        crate::Clob::temporary(self)
    }

    /// Allocate a BLOB locator for a non-nullable output parameter.
    ///
    /// This allocates only a client-side descriptor. It does not create a
    /// temporary server LOB. Bind the returned locator with [`crate::output`].
    #[inline]
    pub fn output_blob(&self) -> Result<crate::Blob<'_>, Error> {
        crate::Blob::output(self)
    }

    /// Allocate a CLOB locator for a non-nullable output parameter.
    ///
    /// This allocates only a client-side descriptor. It does not create a
    /// temporary server LOB. Bind the returned locator with [`crate::output`].
    #[inline]
    pub fn output_clob(&self) -> Result<crate::Clob<'_>, Error> {
        crate::Clob::output(self)
    }

    #[inline]
    fn with_read_handle<T>(&self, operation: impl FnOnce(&ffi::YacLib, &ConnectionHandle) -> T) -> T {
        // SAFETY: Connection is !Sync. This path only exposes shared handle
        // references to FFI operations that do not mutate connection state.
        unsafe { operation(self.lib, &*self.handle.get()) }
    }

    // --- runtime conn attr getters/setters ---

    /// Get the connection's auto-commit mode.
    ///
    /// New connections use manual commit (`false`) by default. See
    /// [`Self::transaction`] for a scoped transaction on an auto-commit
    /// connection.
    #[inline]
    pub fn auto_commit(&self) -> bool {
        self.with_read_handle(|lib, handle| lib.get_conn_auto_commit(&handle.dbc))
    }

    /// Set the connection's auto-commit mode.
    ///
    /// Changing this attribute does not replace an explicit [`Self::commit`] or
    /// [`Self::rollback`] for pending work.
    #[inline]
    pub fn set_auto_commit(&mut self, enabled: bool) {
        self.with_handle(|lib, dbc| lib.set_conn_auto_commit(dbc, enabled));
    }

    /// Commit the current transaction without changing auto-commit mode.
    ///
    /// Use this to finish work managed directly on a manual-commit connection.
    /// Prefer [`Self::transaction`] when a scoped guard is appropriate.
    #[inline]
    pub fn commit(&self) -> Result<(), Error> {
        self.with_handle(|lib, dbc| lib.commit(dbc))
    }

    /// Roll back the current transaction without changing auto-commit mode.
    ///
    /// Use this to discard work managed directly on a manual-commit connection.
    /// Prefer [`Self::transaction`] when a scoped guard is appropriate.
    #[inline]
    pub fn rollback(&self) -> Result<(), Error> {
        self.with_handle(|lib, dbc| lib.rollback(dbc))
    }

    /// Start a scoped guard for the current connection transaction.
    ///
    /// When auto-commit is enabled, the guard disables it until commit,
    /// rollback, or drop and then restores it. When auto-commit is disabled,
    /// the guard takes ownership of the current connection transaction; its
    /// completion affects any work already pending on the connection. It does
    /// not send `BEGIN`, create a separate server transaction, or support
    /// nesting.
    ///
    /// An unfinished guard attempts to roll back when dropped. The guard
    /// exclusively borrows this connection, so finish result sets and
    /// statements created through it before calling [`Transaction::commit`] or
    /// [`Transaction::rollback`]. If application SQL executes transaction
    /// control statements directly, the application is responsible for keeping
    /// its transaction state consistent with this guard.
    #[inline]
    pub fn transaction(&mut self) -> Transaction<'_> {
        let restore_auto_commit = self.auto_commit();
        if restore_auto_commit {
            self.set_auto_commit(false);
        }
        Transaction::new(self, restore_auto_commit)
    }

    /// Get the transaction isolation level.
    #[inline]
    pub fn transaction_isolation(&self) -> TransactionIsolation {
        match self.with_read_handle(|lib, handle| lib.get_conn_transaction_isolation(&handle.dbc)) {
            ffi::YacTxnIsolation::ReadCommitted => TransactionIsolation::ReadCommitted,
            ffi::YacTxnIsolation::CurrCommitted => TransactionIsolation::CurrentCommitted,
            ffi::YacTxnIsolation::Serializable => TransactionIsolation::Serializable,
        }
    }

    /// Set the transaction isolation level.
    ///
    /// Sends a message to the server and may fail if the session is in a
    /// transaction.
    #[inline]
    pub fn set_transaction_isolation(&mut self, isolation: TransactionIsolation) -> Result<(), Error> {
        let level = match isolation {
            TransactionIsolation::ReadCommitted => ffi::YacTxnIsolation::ReadCommitted,
            TransactionIsolation::CurrentCommitted => ffi::YacTxnIsolation::CurrCommitted,
            TransactionIsolation::Serializable => ffi::YacTxnIsolation::Serializable,
        };
        self.with_handle(|lib, dbc| lib.set_conn_transaction_isolation(dbc, level))
    }

    /// Get whether heartbeat is enabled.
    #[inline]
    pub fn heartbeat_enabled(&self) -> bool {
        self.with_read_handle(|lib, handle| lib.get_conn_heartbeat_enabled(&handle.dbc))
    }

    /// Get the packet size in bytes.
    ///
    /// The packet size is fixed at connect time; this only queries it.
    #[inline]
    pub fn packet_size(&self) -> u32 {
        self.with_read_handle(|lib, handle| lib.get_conn_packet_size(&handle.dbc))
    }

    #[inline]
    pub(crate) fn alloc_stmt(&self) -> Result<ffi::StmtHandle, Error> {
        self.with_handle(|lib, dbc| lib.alloc_stmt(dbc))
    }

    #[inline]
    pub(crate) fn charset_ratios(&self) -> Result<(u32, u32), Error> {
        self.with_read_handle(|lib, handle| {
            Ok((
                lib.get_conn_max_charset_ratio(&handle.dbc)?,
                lib.get_conn_max_ncharset_ratio(&handle.dbc)?,
            ))
        })
    }

    /// Execute non-parameterized SQL that does not return a result set.
    ///
    /// Returns the affected-row count reported by the server. The count for DDL
    /// and other statements without affected rows is server-defined.
    #[inline]
    pub fn execute(&self, sql: &str) -> Result<ExecResult, Error> {
        let mut stmt = Statement::new(self)?;
        stmt.direct_execute(sql)
    }

    /// Execute non-parameterized SQL that returns a streaming result set.
    ///
    /// The returned result set shares this connection until it is dropped or
    /// [`ResultSet::finish`]ed. Other statements on the same connection may
    /// execute while it is active. Use [`Self::execute`] for SQL that does not
    /// return rows.
    #[inline]
    pub fn query(&self, sql: &str) -> Result<ResultSet<'_, '_>, Error> {
        Statement::new(self)?.direct_query(sql)
    }

    /// Prepare SQL for repeated execution on this connection.
    #[inline]
    pub fn prepare(&self, sql: &str) -> Result<Statement<'_>, Error> {
        let mut statement = Statement::new(self)?;
        statement.prepare(sql)?;
        Ok(statement)
    }

    /// Execute parameterized SQL using a temporary prepared statement.
    ///
    /// Parameters are supplied as an array, slice, or `Vec` of [`BindParam`]
    /// values.
    #[inline]
    pub fn execute_with<'conn, 'param>(
        &'conn self,
        sql: &str,
        params: impl AsMut<[BindParam<'conn, 'param>]>,
    ) -> Result<ExecResult, Error>
    where
        'conn: 'param,
    {
        self.prepare(sql)?.execute(params)
    }

    /// Execute parameterized SQL returning rows using a temporary prepared statement.
    ///
    /// Parameters are supplied as an array, slice, or `Vec` of [`BindParam`]
    /// values.
    #[inline]
    pub fn query_with<'conn, 'param>(
        &'conn self,
        sql: &str,
        params: impl AsMut<[BindParam<'conn, 'param>]>,
    ) -> Result<ResultSet<'conn, 'conn>, Error>
    where
        'conn: 'param,
    {
        let mut params = params;
        self.prepare(sql)?.query_owned(params.as_mut())
    }

    /// Execute named parameterized SQL using a temporary prepared statement.
    ///
    /// Names are passed without a SQL placeholder prefix: use `value` for
    /// `:value` in SQL. Parameters are supplied as an array, slice, or `Vec`.
    #[inline]
    pub fn execute_named_with<'conn, 'name, 'param>(
        &'conn self,
        sql: &str,
        params: impl AsMut<[NamedBindParam<'conn, 'name, 'param>]>,
    ) -> Result<ExecResult, Error>
    where
        'conn: 'param,
    {
        self.prepare(sql)?.execute_named(params)
    }

    /// Execute named parameterized SQL returning rows using a temporary prepared statement.
    ///
    /// Names are passed without a SQL placeholder prefix: use `value` for
    /// `:value` in SQL. Parameters are supplied as an array, slice, or `Vec`.
    #[inline]
    pub fn query_named_with<'conn, 'name, 'param>(
        &'conn self,
        sql: &str,
        params: impl AsMut<[NamedBindParam<'conn, 'name, 'param>]>,
    ) -> Result<ResultSet<'conn, 'conn>, Error>
    where
        'conn: 'param,
    {
        let mut params = params;
        self.prepare(sql)?.query_named_owned(params.as_mut())
    }

    /// Execute a query that must return exactly one row and map it.
    ///
    /// Returns [`Error::RowNotFound`] for zero rows and [`Error::TooManyRows`]
    /// for more than one row. The mapping function runs after the first row is
    /// fetched but before the second-row check, so it can run even when this
    /// method ultimately returns [`Error::TooManyRows`].
    ///
    /// The result set is released before this method returns. If query, fetch,
    /// or mapping fails, that error is returned even when release also fails.
    /// A release failure is returned only after a successful operation.
    pub fn query_one_map<T>(
        &self,
        sql: &str,
        map: impl FnOnce(&crate::result_set::Row<'_, '_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut rows = self.query(sql)?;
        let operation = (|| {
            let row = rows.fetch()?.ok_or(Error::RowNotFound)?;
            let value = map(&row)?;
            if rows.fetch()?.is_some() {
                return Err(Error::TooManyRows);
            }
            Ok(value)
        })();
        finish_result(operation, rows.finish())
    }

    /// Execute a query that returns zero or one row and map it.
    ///
    /// Returns `Ok(None)` for zero rows and [`Error::TooManyRows`] for more than
    /// one row. The mapping function runs after the first row is fetched but
    /// before the second-row check, so it can run even when this method
    /// ultimately returns [`Error::TooManyRows`].
    ///
    /// The result set is released before this method returns. If query, fetch,
    /// or mapping fails, that error is returned even when release also fails.
    /// A release failure is returned only after a successful operation.
    pub fn query_opt_map<T>(
        &self,
        sql: &str,
        map: impl FnOnce(&crate::result_set::Row<'_, '_>) -> Result<T, Error>,
    ) -> Result<Option<T>, Error> {
        let mut rows = self.query(sql)?;
        let operation = (|| {
            let Some(row) = rows.fetch()? else {
                return Ok(None);
            };
            let value = map(&row)?;
            if rows.fetch()?.is_some() {
                return Err(Error::TooManyRows);
            }
            Ok(Some(value))
        })();
        finish_result(operation, rows.finish())
    }
}

#[inline]
fn finish_result<T>(operation: Result<T, Error>, finish: Result<(), Error>) -> Result<T, Error> {
    match operation {
        Ok(value) => finish.map(|()| value),
        Err(error) => Err(error),
    }
}

/// Transaction isolation level for a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionIsolation {
    /// Read committed.
    ReadCommitted,
    /// Current committed, a YashanDB-specific level.
    CurrentCommitted,
    /// Serializable.
    Serializable,
}

/// Builder for [`Connection`], allowing connection attributes to be set before
/// connecting.
pub struct ConnectionBuilder {
    login_timeout: Option<u32>,
    packet_size: Option<u32>,
    auto_commit: Option<bool>,
    transaction_isolation: Option<TransactionIsolation>,
    heartbeat_enabled: Option<bool>,
}

impl ConnectionBuilder {
    #[inline]
    const fn new() -> Self {
        Self {
            login_timeout: None,
            packet_size: None,
            auto_commit: None,
            transaction_isolation: None,
            heartbeat_enabled: None,
        }
    }
}

/// RAII guard that frees env/dbc handles on drop.
struct ConnectGuard<'a> {
    lib: &'a ffi::YacLib,
    env: Option<ffi::EnvHandle>,
    dbc: Option<ffi::DbcHandle>,
    connected: bool,
}

impl<'a> ConnectGuard<'a> {
    #[inline]
    const fn new(lib: &'a ffi::YacLib) -> Self {
        Self {
            lib,
            env: None,
            dbc: None,
            connected: false,
        }
    }

    /// Record that a connection has been established, so a subsequent drop
    /// disconnects (rather than freeing a live session) before freeing dbc.
    #[inline]
    fn mark_connected(&mut self) {
        self.connected = true;
    }

    #[inline]
    fn set_env(&mut self, env: ffi::EnvHandle) -> &mut ffi::EnvHandle {
        self.env = Some(env);
        self.env.as_mut().unwrap()
    }

    #[inline]
    fn set_dbc(&mut self, dbc: ffi::DbcHandle) -> &mut ffi::DbcHandle {
        self.dbc = Some(dbc);
        self.dbc.as_mut().unwrap()
    }

    #[inline]
    fn dbc_mut(&mut self) -> &mut ffi::DbcHandle {
        self.dbc.as_mut().unwrap()
    }

    #[inline]
    fn into_handles(mut self) -> (ffi::EnvHandle, ffi::DbcHandle) {
        let env = self.env.take().unwrap();
        let dbc = self.dbc.take().unwrap();
        std::mem::forget(self);
        (env, dbc)
    }
}

impl Drop for ConnectGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        if let Some(mut dbc) = self.dbc.take() {
            if self.connected {
                self.lib.disconnect(&mut dbc);
            }
            self.lib.free_dbc(&mut dbc);
        }
        if let Some(mut env) = self.env.take() {
            self.lib.free_env(&mut env);
        }
    }
}

impl ConnectionBuilder {
    /// Set the login timeout in seconds.
    ///
    /// Only applies to the login process; the default is 300 seconds. Some
    /// client library versions reject this attribute (`unknown attribute id`),
    /// so the error is returned at connect time.
    #[inline]
    pub const fn login_timeout(mut self, seconds: u32) -> Self {
        self.login_timeout = Some(seconds);
        self
    }

    /// Set the packet size in bytes.
    ///
    /// Must be within the range [64 KiB, 32 MiB]; otherwise
    /// [`Error::InvalidArgument`] is returned at connect time.
    #[inline]
    pub const fn packet_size(mut self, bytes: u32) -> Self {
        self.packet_size = Some(bytes);
        self
    }

    /// Set the auto-commit mode used by the new connection.
    ///
    /// The default is manual commit (`false`).
    #[inline]
    pub const fn auto_commit(mut self, enabled: bool) -> Self {
        self.auto_commit = Some(enabled);
        self
    }

    /// Set the transaction isolation level.
    #[inline]
    pub const fn transaction_isolation(mut self, isolation: TransactionIsolation) -> Self {
        self.transaction_isolation = Some(isolation);
        self
    }

    /// Set whether heartbeat is enabled.
    #[inline]
    pub const fn heartbeat_enabled(mut self, enabled: bool) -> Self {
        self.heartbeat_enabled = Some(enabled);
        self
    }

    /// Establish a connection with the configured attributes applied.
    pub fn connect(self, url: &str, username: &str, password: &str) -> Result<Connection, Error> {
        let lib = library::library(None)?;

        let mut g = ConnectGuard::new(lib);
        let dbc = {
            let env = g.set_env(lib.alloc_env()?);
            set_env_attrs(lib, env);
            let dbc = lib.alloc_dbc(env)?;
            g.set_dbc(dbc)
        };

        // Pre-connect attrs: login_timeout, packet_size, auto_commit, heartbeat_enabled.
        if let Some(seconds) = self.login_timeout {
            lib.set_conn_login_timeout(dbc, seconds)?;
        }
        if let Some(bytes) = self.packet_size {
            const MIN_PACKET_SIZE: u32 = 64 * 1024;
            const MAX_PACKET_SIZE: u32 = 32 * 1024 * 1024;
            if !(MIN_PACKET_SIZE..=MAX_PACKET_SIZE).contains(&bytes) {
                return Err(Error::InvalidArgument(format!(
                    "packet size {bytes} is out of range [{MIN_PACKET_SIZE}, {MAX_PACKET_SIZE}]"
                )));
            }
            lib.set_conn_packet_size(dbc, bytes);
        }
        lib.set_conn_auto_commit(dbc, self.auto_commit.unwrap_or(false));
        if let Some(enabled) = self.heartbeat_enabled {
            lib.set_conn_heartbeat_enabled(dbc, enabled);
        }

        lib.connect(dbc, url, username, password)?;

        // Isolation is set on a live session and may fail if the session is in a
        // transaction. If it fails after a successful connect, the guard's Drop
        // disconnects (connected was marked above) before freeing the handles.
        g.mark_connected();
        if let Some(isolation) = self.transaction_isolation {
            lib.set_conn_transaction_isolation(
                g.dbc_mut(),
                match isolation {
                    TransactionIsolation::ReadCommitted => ffi::YacTxnIsolation::ReadCommitted,
                    TransactionIsolation::CurrentCommitted => ffi::YacTxnIsolation::CurrCommitted,
                    TransactionIsolation::Serializable => ffi::YacTxnIsolation::Serializable,
                },
            )?;
        }

        let (env, dbc) = g.into_handles();
        Ok(Connection {
            lib,
            handle: UnsafeCell::new(ConnectionHandle { env, dbc }),
        })
    }
}

/// Configure the env attributes that identify this client before connecting.
///
/// All three are supported by the baseline client library, so the per-attr
/// setters `.expect()` internally; this cannot fail.
fn set_env_attrs(lib: &ffi::YacLib, env: &mut ffi::EnvHandle) {
    lib.set_env_charset_code(env, ffi::YacCharsetCode::UTF8);
    lib.set_env_client_driver(env, "YashanDB Rust Driver");
    lib.set_env_software_version(env, env!("CARGO_PKG_VERSION"));
}

/// SAFETY: `Connection` owns its raw handles exclusively and is `!Sync` (the raw
/// pointers are not `Sync`), so the C driver is only ever invoked from a single
/// thread at a time. Moving it between threads (`Send`) is safe because each
/// connection uses its own distinct env/dbc handles and the C functions do not
/// require thread affinity (diagnostics use per-thread buffers read on the same
/// thread that made the call).
unsafe impl Send for Connection {}

impl Drop for Connection {
    #[inline]
    fn drop(&mut self) {
        self.with_connection_handle(|lib, handle| {
            lib.disconnect(&mut handle.dbc);
            lib.free_dbc(&mut handle.dbc);
            lib.free_env(&mut handle.env);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::YacTxnIsolation;

    fn internal_error(message: &str) -> Error {
        Error::Internal(message.to_owned())
    }

    #[test]
    fn txn_isolation_conversion_roundtrip() {
        for (rust, raw) in [
            (TransactionIsolation::ReadCommitted, YacTxnIsolation::ReadCommitted),
            (TransactionIsolation::CurrentCommitted, YacTxnIsolation::CurrCommitted),
            (TransactionIsolation::Serializable, YacTxnIsolation::Serializable),
        ] {
            let raw_from_rust = match rust {
                TransactionIsolation::ReadCommitted => YacTxnIsolation::ReadCommitted,
                TransactionIsolation::CurrentCommitted => YacTxnIsolation::CurrCommitted,
                TransactionIsolation::Serializable => YacTxnIsolation::Serializable,
            };
            assert_eq!(raw_from_rust as i32, raw as i32);
            let rust_from_raw = match raw {
                YacTxnIsolation::ReadCommitted => TransactionIsolation::ReadCommitted,
                YacTxnIsolation::CurrCommitted => TransactionIsolation::CurrentCommitted,
                YacTxnIsolation::Serializable => TransactionIsolation::Serializable,
            };
            assert_eq!(rust_from_raw, rust);
        }
    }

    #[test]
    fn finish_result_returns_value_when_operation_and_release_succeed() {
        assert!(matches!(finish_result(Ok(42), Ok(())), Ok(42)));
    }

    #[test]
    fn finish_result_returns_release_error_after_successful_operation() {
        let result = finish_result(Ok(()), Err(internal_error("release failed")));
        assert!(matches!(result, Err(Error::Internal(message)) if message == "release failed"));
    }

    #[test]
    fn finish_result_preserves_operation_error_when_release_also_fails() {
        let result = finish_result::<()>(
            Err(internal_error("query failed")),
            Err(internal_error("release failed")),
        );
        assert!(matches!(result, Err(Error::Internal(message)) if message == "query failed"));
    }
}
