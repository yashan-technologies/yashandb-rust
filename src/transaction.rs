//! Scoped transaction guard.

use crate::conn::Connection;
use crate::error::Error;
use crate::param::{BindParam, NamedBindParam};
use crate::result_set::ResultSet;
use crate::stmt::{ExecResult, Statement};

/// A scoped transaction that exclusively owns access to its connection.
///
/// When created from an auto-commit connection, it disables auto-commit while
/// alive and restores it after an explicit commit, rollback, or drop. When
/// created from a manual-commit connection, it guards the entire current
/// connection transaction, including work pending before the guard was created.
/// It does not create a separate server transaction or support nested
/// transactions.
///
/// Unless it is explicitly committed or rolled back, dropping this guard
/// attempts to roll back the current connection transaction. Drop cleanup
/// ignores rollback failures. The guard exclusively owns access to its
/// connection, while ordinary operations use shared borrows so multiple
/// statements or result sets can be created. A [`ResultSet`] or [`Statement`]
/// created from it must be finished or dropped before it can be committed or
/// rolled back. Applications that execute
/// transaction-control SQL directly are responsible for maintaining a
/// transaction state consistent with the guard.
pub struct Transaction<'conn> {
    conn: &'conn mut Connection,
    restore_auto_commit: bool,
    completed: bool,
}

impl<'conn> Transaction<'conn> {
    #[inline]
    pub(crate) const fn new(conn: &'conn mut Connection, restore_auto_commit: bool) -> Self {
        Self {
            conn,
            restore_auto_commit,
            completed: false,
        }
    }

    /// Execute non-parameterized SQL that does not return a result set.
    ///
    /// See [`Connection::execute`] for the execution and affected-row semantics.
    #[inline]
    pub fn execute(&self, sql: &str) -> Result<ExecResult, Error> {
        self.conn.execute(sql)
    }

    /// Create an empty temporary binary LOB owned by this transaction.
    #[inline]
    pub fn temporary_blob(&self) -> Result<crate::Blob<'_>, Error> {
        self.conn.temporary_blob()
    }

    /// Create an empty temporary character LOB owned by this transaction.
    #[inline]
    pub fn temporary_clob(&self) -> Result<crate::Clob<'_>, Error> {
        self.conn.temporary_clob()
    }

    /// Allocate a BLOB locator for a non-nullable output parameter.
    ///
    /// See [`Connection::output_blob`].
    #[inline]
    pub fn output_blob(&self) -> Result<crate::Blob<'_>, Error> {
        self.conn.output_blob()
    }

    /// Allocate a CLOB locator for a non-nullable output parameter.
    ///
    /// See [`Connection::output_clob`].
    #[inline]
    pub fn output_clob(&self) -> Result<crate::Clob<'_>, Error> {
        self.conn.output_clob()
    }

    /// Execute non-parameterized SQL that returns a streaming result set.
    ///
    /// See [`Connection::query`] for query semantics.
    ///
    /// The result set borrows this transaction until it is finished or dropped.
    #[inline]
    pub fn query(&self, sql: &str) -> Result<ResultSet<'_, '_>, Error> {
        self.conn.query(sql)
    }

    /// Prepare SQL for repeated execution on this transaction's connection.
    ///
    /// See [`Connection::prepare`] for prepared-statement semantics.
    ///
    /// The returned statement borrows this transaction until it is dropped or
    /// finished.
    #[inline]
    pub fn prepare(&self, sql: &str) -> Result<Statement<'_>, Error> {
        self.conn.prepare(sql)
    }

    /// Execute parameterized SQL using a temporary prepared statement.
    ///
    /// See [`Connection::execute_with`] for parameter binding semantics.
    #[inline]
    pub fn execute_with<'txn, 'param>(
        &'txn self,
        sql: &str,
        params: impl AsMut<[BindParam<'txn, 'param>]>,
    ) -> Result<ExecResult, Error>
    where
        'txn: 'param,
    {
        self.conn.execute_with(sql, params)
    }

    /// Execute parameterized SQL returning rows using a temporary prepared statement.
    ///
    /// See [`Connection::query_with`] for parameter binding and query semantics.
    ///
    /// The result set borrows this transaction until it is finished or dropped.
    #[inline]
    pub fn query_with<'txn, 'param>(
        &'txn self,
        sql: &str,
        params: impl AsMut<[BindParam<'txn, 'param>]>,
    ) -> Result<ResultSet<'txn, 'txn>, Error>
    where
        'txn: 'param,
    {
        self.conn.query_with(sql, params)
    }

    /// Execute named parameterized SQL using a temporary prepared statement.
    ///
    /// See [`Connection::execute_named_with`] for named parameter binding semantics.
    #[inline]
    pub fn execute_named_with<'txn, 'name, 'param>(
        &'txn self,
        sql: &str,
        params: impl AsMut<[NamedBindParam<'txn, 'name, 'param>]>,
    ) -> Result<ExecResult, Error>
    where
        'txn: 'param,
    {
        self.conn.execute_named_with(sql, params)
    }

    /// Execute named parameterized SQL returning rows using a temporary prepared statement.
    ///
    /// See [`Connection::query_named_with`] for named parameter binding and query
    /// semantics.
    ///
    /// The result set borrows this transaction until it is finished or dropped.
    #[inline]
    pub fn query_named_with<'txn, 'name, 'param>(
        &'txn self,
        sql: &str,
        params: impl AsMut<[NamedBindParam<'txn, 'name, 'param>]>,
    ) -> Result<ResultSet<'txn, 'txn>, Error>
    where
        'txn: 'param,
    {
        self.conn.query_named_with(sql, params)
    }

    /// Execute a query that must return exactly one row and map it.
    ///
    /// See [`Connection::query_one_map`] for cardinality, mapping, and cleanup
    /// semantics.
    #[inline]
    pub fn query_one_map<T>(
        &self,
        sql: &str,
        map: impl FnOnce(&crate::result_set::Row<'_, '_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.conn.query_one_map(sql, map)
    }

    /// Execute a query that returns zero or one row and map it.
    ///
    /// See [`Connection::query_opt_map`] for cardinality, mapping, and cleanup
    /// semantics.
    #[inline]
    pub fn query_opt_map<T>(
        &self,
        sql: &str,
        map: impl FnOnce(&crate::result_set::Row<'_, '_>) -> Result<T, Error>,
    ) -> Result<Option<T>, Error> {
        self.conn.query_opt_map(sql, map)
    }

    /// Commit the current connection transaction and consume this guard.
    ///
    /// If the connection used auto-commit when this guard was created, restores
    /// auto-commit after committing.
    #[inline]
    pub fn commit(mut self) -> Result<(), Error> {
        self.finish(true)
    }

    /// Roll back the current connection transaction and consume this guard.
    ///
    /// If the connection used auto-commit when this guard was created, restores
    /// auto-commit after rolling back.
    #[inline]
    pub fn rollback(mut self) -> Result<(), Error> {
        self.finish(false)
    }

    #[inline]
    fn finish(&mut self, commit: bool) -> Result<(), Error> {
        let operation = if commit {
            self.conn.commit()
        } else {
            self.conn.rollback()
        };
        self.completed = true;
        if self.restore_auto_commit {
            self.conn.set_auto_commit(true);
        }
        operation
    }
}

impl Drop for Transaction<'_> {
    #[inline]
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.conn.rollback();
            if self.restore_auto_commit {
                self.conn.set_auto_commit(true);
            }
        }
    }
}
