//! Integration tests for connection and scoped transaction operations.

mod common;

use std::sync::Mutex;

use yashandb::Connection;

static LIB_LOCK: Mutex<()> = Mutex::new(());

fn setup() -> Option<(String, String, String)> {
    common::lib_path()?;
    common::require_library();
    common::conn_credentials()
}

fn skip_msg() -> &'static str {
    "skipping transaction integration test: database environment is not configured"
}

fn connect(url: &str, user: &str, password: &str) -> Connection {
    Connection::builder()
        .auto_commit(true)
        .connect(url, user, password)
        .expect("database connection should succeed")
}

#[test]
fn commit_persists_changes() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_commit_test";
    let mut conn = connect(&url, &user, &password);
    let mut observer = connect(&url, &user, &password);
    prepare_table(&mut conn, table);

    let mut tx = conn.transaction();
    tx.execute(&format!("insert into {table} values (1, 'one'), (2, 'two')"))
        .expect("transaction insert should succeed");
    assert_eq!(count_rows(&mut observer, table), 0);
    tx.commit().expect("commit should succeed");
    assert_eq!(count_rows(&mut observer, table), 2);
    assert!(conn.auto_commit());
    cleanup(&mut conn, table);
}

#[test]
fn rollback_discards_changes() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_rollback_test";
    let mut conn = connect(&url, &user, &password);
    let mut observer = connect(&url, &user, &password);
    prepare_table(&mut conn, table);

    let mut tx = conn.transaction();
    tx.execute(&format!("insert into {table} values (1, 'one')"))
        .expect("transaction insert should succeed");
    assert_eq!(count_rows(&mut observer, table), 0);
    tx.rollback().expect("rollback should succeed");
    assert_eq!(count_rows(&mut observer, table), 0);
    assert!(conn.auto_commit());
    cleanup(&mut conn, table);
}

#[test]
fn drop_rolls_back_changes() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_drop_test";
    let mut conn = connect(&url, &user, &password);
    let mut observer = connect(&url, &user, &password);
    prepare_table(&mut conn, table);

    {
        let mut tx = conn.transaction();
        tx.execute(&format!("insert into {table} values (1, 'one')"))
            .expect("transaction insert should succeed");
    }
    assert_eq!(count_rows(&mut observer, table), 0);
    assert!(conn.auto_commit());
    cleanup(&mut conn, table);
}

#[test]
fn post_transaction_statement_uses_restored_auto_commit() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_restored_autocommit_test";
    let mut conn = connect(&url, &user, &password);
    let mut observer = connect(&url, &user, &password);
    prepare_table(&mut conn, table);

    let mut tx = conn.transaction();
    tx.execute(&format!("insert into {table} values (1, 'transaction')"))
        .expect("transaction insert should succeed");
    tx.rollback().expect("rollback should succeed");
    assert!(conn.auto_commit());

    conn.execute(&format!("insert into {table} values (2, 'auto commit')"))
        .expect("post-transaction insert should succeed");
    assert_eq!(count_rows(&mut observer, table), 1);
    cleanup(&mut conn, table);
}

#[test]
fn manual_mode_guard_includes_existing_work() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_manual_test";
    let mut conn = Connection::connect(&url, &user, &password).expect("database connection should succeed");
    let mut observer = connect(&url, &user, &password);
    prepare_table(&mut observer, table);

    conn.execute(&format!("insert into {table} values (1, 'before guard')"))
        .expect("pre-guard insert should succeed");
    let mut tx = conn.transaction();
    tx.execute(&format!("insert into {table} values (2, 'inside guard')"))
        .expect("transaction insert should succeed");
    assert_eq!(count_rows(&mut observer, table), 0);
    tx.rollback().expect("rollback should succeed");
    assert_eq!(count_rows(&mut observer, table), 0);
    assert!(!conn.auto_commit());
    cleanup(&mut observer, table);
}

#[test]
fn manual_commit_and_rollback() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_manual_connection_test";
    let mut conn = Connection::connect(&url, &user, &password).expect("database connection should succeed");
    let mut observer = connect(&url, &user, &password);
    prepare_table(&mut observer, table);

    conn.execute(&format!("insert into {table} values (1, 'committed')"))
        .expect("manual-mode insert should succeed");
    assert_eq!(count_rows(&mut observer, table), 0);
    conn.commit().expect("connection commit should succeed");
    assert_eq!(count_rows(&mut observer, table), 1);
    assert!(!conn.auto_commit());

    conn.execute(&format!("insert into {table} values (2, 'rolled back')"))
        .expect("manual-mode insert should succeed");
    assert_eq!(count_rows(&mut observer, table), 1);
    conn.rollback().expect("connection rollback should succeed");
    assert_eq!(count_rows(&mut observer, table), 1);
    assert!(!conn.auto_commit());
    cleanup(&mut observer, table);
}

#[test]
fn default_auto_commit_is_disabled() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let conn = Connection::connect(&url, &user, &password).expect("database connection should succeed");
    assert!(!conn.auto_commit());
}

fn prepare_table(conn: &mut Connection, table: &str) {
    let _ = conn.execute(&format!("drop table {table}"));
    conn.execute(&format!("create table {table} (id integer, name varchar(32))"))
        .expect("test table creation should succeed");
}

fn count_rows(conn: &mut Connection, table: &str) -> i64 {
    conn.query_one_map(&format!("select count(*) from {table}"), |row| row.get(0))
        .expect("row count query should succeed")
}

fn cleanup(conn: &mut Connection, table: &str) {
    conn.execute(&format!("drop table {table}"))
        .expect("test table cleanup should succeed");
}
