//! Integration tests for connection and scoped transaction operations.

mod common;

use std::sync::Mutex;

use yashandb::{Connection, input, named};

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

    let tx = conn.transaction();
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

    let tx = conn.transaction();
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
        let tx = conn.transaction();
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

    let tx = conn.transaction();
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
    let tx = conn.transaction();
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
    let conn = Connection::connect(&url, &user, &password).expect("database connection should succeed");
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

#[test]
fn transaction_supports_multiple_active_statements() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let mut conn = connect(&url, &user, &password);
    let tx = conn.transaction();
    let mut first_statement = tx
        .prepare("select 1 from dual")
        .expect("first statement should prepare");
    let mut second_statement = tx
        .prepare("select 2 from dual")
        .expect("second statement should prepare");
    let mut first = first_statement.query([]).expect("first query should succeed");
    let mut second = second_statement.query([]).expect("second query should succeed");

    assert_eq!(first.fetch().unwrap().unwrap().get::<i32>(0).unwrap(), 1);
    assert_eq!(second.fetch().unwrap().unwrap().get::<i32>(0).unwrap(), 2);
    first.finish().expect("first result cleanup should succeed");
    second.finish().expect("second result cleanup should succeed");
    drop(first_statement);
    drop(second_statement);
    tx.commit().expect("commit should succeed");
}

#[test]
fn transaction_forwards_query_and_binding_operations() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let Some((url, user, password)) = setup() else {
        eprintln!("{}", skip_msg());
        return;
    };
    let table = "rust_tx_forwarding_test";
    let mut conn = connect(&url, &user, &password);
    let _ = conn.execute(&format!("drop table {table}"));
    let tx = conn.transaction();

    tx.execute(&format!("create table {table} (id integer, name varchar(32))"))
        .expect("table creation should succeed");
    tx.execute_with(
        &format!("insert into {table} values (?, ?)"),
        [input(1_i32), input("positional")],
    )
    .expect("positional execute should succeed");
    tx.execute_named_with(
        &format!("insert into {table} values (:id, :name)"),
        [named(c"id", input(2_i32)), named(c"name", input("named"))],
    )
    .expect("named execute should succeed");

    let mut direct_rows = tx
        .query(&format!("select id from {table} order by id"))
        .expect("query should succeed");
    assert_eq!(direct_rows.fetch().unwrap().unwrap().get::<i32>(0).unwrap(), 1);
    assert_eq!(direct_rows.fetch().unwrap().unwrap().get::<i32>(0).unwrap(), 2);
    direct_rows.finish().expect("query cleanup should succeed");

    let mut positional_rows = tx
        .query_with(&format!("select name from {table} where id = ?"), [input(1_i32)])
        .expect("positional query should succeed");
    assert_eq!(
        positional_rows.fetch().unwrap().unwrap().get::<String>(0).unwrap(),
        "positional"
    );
    positional_rows.finish().expect("positional cleanup should succeed");

    let mut named_rows = tx
        .query_named_with(
            &format!("select name from {table} where id = :id"),
            [named(c"id", input(2_i32))],
        )
        .expect("named query should succeed");
    assert_eq!(named_rows.fetch().unwrap().unwrap().get::<String>(0).unwrap(), "named");
    named_rows.finish().expect("named cleanup should succeed");

    assert_eq!(
        tx.query_one_map(&format!("select name from {table} where id = 1"), |row| row
            .get::<String>(0))
            .expect("query_one_map should succeed"),
        "positional"
    );
    assert_eq!(
        tx.query_opt_map(&format!("select name from {table} where id = 3"), |row| row
            .get::<String>(0))
            .expect("query_opt_map should succeed"),
        None
    );

    tx.commit().expect("commit should succeed");
    cleanup(&mut conn, table);
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
