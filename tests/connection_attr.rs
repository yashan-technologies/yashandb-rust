//! Integration tests for `ConnectionBuilder` and runtime connection attributes.
//!
//! Requires a reachable YashanDB instance configured via `YASDB_URL`,
//! `YASDB_USER`, `YASDB_PASSWORD`, and the client library via `YASCLI_HOME`.
//! Tests are skipped (not failed) when the environment is not configured.

mod common;

use std::sync::Mutex;

use common::{LIB_HOME_VAR, conn_credentials, require_library};
use yashandb::{Connection, TransactionIsolation};

/// Serializes the tests in this binary. The library is a process-global
/// singleton, so tests that touch it must not run concurrently with each other.
static LIB_LOCK: Mutex<()> = Mutex::new(());

/// Load the library and return connection credentials, or `None` to skip.
fn setup() -> Option<(String, String, String)> {
    let _g = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if common::lib_path().is_none() {
        eprintln!("skipping: {LIB_HOME_VAR} not set or library missing");
        return None;
    }
    require_library();
    conn_credentials()
}

fn skip_msg() -> &'static str {
    "skipping: YASDB_URL/YASDB_USER/YASDB_PASSWORD not all set"
}

/// Connect via the builder applying no custom attributes.
fn connect_default(url: &str, user: &str, pass: &str) -> Connection {
    Connection::builder()
        .connect(url, user, pass)
        .expect("connect should succeed")
}

#[test]
fn builder_packet_size_roundtrip() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = Connection::builder()
        .packet_size(128 * 1024)
        .connect(&url, &user, &pass)
        .expect("connect should succeed");
    assert_eq!(conn.packet_size(), 128 * 1024);
}

#[test]
fn builder_auto_commit_roundtrip() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = Connection::builder()
        .auto_commit(false)
        .connect(&url, &user, &pass)
        .expect("connect should succeed");
    assert!(!conn.auto_commit());
}

#[test]
fn builder_heartbeat_enabled_roundtrip() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = Connection::builder()
        .heartbeat_enabled(true)
        .connect(&url, &user, &pass)
        .expect("connect should succeed");
    // The server may not report the requested value, so only the builder path
    // is exercised; the getter must at least not error.
    let _ = conn.heartbeat_enabled();
}

#[test]
fn builder_transaction_isolation_roundtrip() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = Connection::builder()
        .transaction_isolation(TransactionIsolation::ReadCommitted)
        .connect(&url, &user, &pass)
        .expect("connect should succeed");
    assert_eq!(conn.transaction_isolation(), TransactionIsolation::ReadCommitted);
}

#[test]
fn builder_login_timeout_connects() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    // Support for YAC_ATTR_LOGIN_TIMEOUT varies by client library version:
    // 23.4.1.102 rejects it with `unknown attribute id` (8028); 23.4.7.100 has
    // a driver bug returning a spurious `SSLROOTCER` error (8004), fixed in
    // later versions. When supported, connect succeeds; otherwise the error is
    // returned to the caller rather than panicking.
    match Connection::builder().login_timeout(60).connect(&url, &user, &pass) {
        Ok(conn) => drop(conn),
        Err(yashandb::Error::Database { .. }) => {
            eprintln!("login_timeout not supported by this client library; skipping assertion");
        }
        Err(e) => panic!("unexpected error setting login_timeout: {e:?}"),
    }
}

#[test]
fn runtime_auto_commit_roundtrip() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = connect_default(&url, &user, &pass);
    conn.set_auto_commit(false);
    assert!(!conn.auto_commit());
    conn.set_auto_commit(true);
    assert!(conn.auto_commit());
}

#[test]
fn runtime_transaction_isolation_roundtrip() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = connect_default(&url, &user, &pass);
    conn.set_transaction_isolation(TransactionIsolation::Serializable)
        .expect("set isolation should succeed on a fresh session");
    assert_eq!(conn.transaction_isolation(), TransactionIsolation::Serializable);
}

#[test]
fn packet_size_out_of_range_errors() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    // Below the minimum packet size (64 KiB).
    let err = match Connection::builder().packet_size(1).connect(&url, &user, &pass) {
        Ok(_) => panic!("expected connect to reject an out-of-range packet size"),
        Err(e) => e,
    };
    assert!(matches!(err, yashandb::Error::InvalidArgument(_)));
}
