//! Integration tests for `Connection::connect` / disconnect.
//!
//! Requires a reachable YashanDB instance configured via `YASDB_URL`,
//! `YASDB_USER`, `YASDB_PASSWORD`, and the client library via `YASCLI_HOME`.
//! Tests are skipped (not failed) when the environment is not configured.

mod common;

use common::{LIB_HOME_VAR, conn_credentials, require_library};

/// Load the library and return connection credentials, or `None` to skip.
fn setup() -> Option<(String, String, String)> {
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

#[test]
fn connect_ok() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let conn = yashandb::Connection::connect(&url, &user, &pass).expect("connect should succeed");
    drop(conn); // Drop must disconnect and free handles without panic.
}

#[test]
fn connect_wrong_password_errors() {
    let Some((url, user, _)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let err = match yashandb::Connection::connect(&url, &user, "wrong-password") {
        Ok(_) => panic!("expected connect to fail with a wrong password"),
        Err(e) => e,
    };
    match &err {
        yashandb::Error::Database { code, message, .. } => {
            assert!(*code != 0, "expected a non-zero error code, got {code}");
            assert!(!message.is_empty(), "expected a non-empty error message");
        }
        other => panic!("expected Database error, got {other:?}"),
    }
}

#[test]
fn connect_unreachable_errors() {
    let Some(_) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    // A local address that is guaranteed not to be listening.
    let err = match yashandb::Connection::connect("127.0.0.1:1", "user", "pass") {
        Ok(_) => panic!("expected connect to fail for an unreachable address"),
        Err(e) => e,
    };
    assert!(matches!(err, yashandb::Error::Database { .. }));
}

#[test]
fn connect_sequential() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    for _ in 0..3 {
        let conn = yashandb::Connection::connect(&url, &user, &pass).expect("connect should succeed");
        drop(conn);
    }
}

#[test]
fn connect_concurrent() {
    let Some((url, user, pass)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    // Concurrent calls are supported, including a concurrent lazy first load.
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let url = url.clone();
            let user = user.clone();
            let pass = pass.clone();
            std::thread::spawn(move || {
                let conn = yashandb::Connection::connect(&url, &user, &pass).expect("connect should succeed");
                drop(conn);
            })
        })
        .collect();
    for h in handles {
        h.join().expect("connection thread panicked");
    }
}
