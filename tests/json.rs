//! Integration tests for JSON queries and parameter binding.

mod common;

use common::{LIB_HOME_VAR, conn_credentials, require_library, test_object_name};
use yashandb::{Connection, DataType, DataTypeInfo, Error, Yason, YasonBuf, input, named, output};

fn setup() -> Option<Connection> {
    if common::lib_path().is_none() {
        eprintln!("skipping: {LIB_HOME_VAR} not set or library missing");
        return None;
    }
    let Some((url, user, password)) = conn_credentials() else {
        eprintln!("skipping: YASDB_URL/YASDB_USER/YASDB_PASSWORD not all set");
        return None;
    };
    require_library();
    Some(Connection::connect(&url, &user, &password).expect("database connection failed"))
}

fn yason(json: impl AsRef<str>) -> YasonBuf {
    YasonBuf::parse(json, false).expect("valid JSON should encode as YASON")
}

fn create_json_table(conn: &Connection, prefix: &str) -> String {
    let table = test_object_name(prefix);
    conn.execute(&format!("create table {table} (id integer, value json)"))
        .expect("JSON table should be created");
    table
}

fn drop_table(conn: &Connection, table: &str) {
    conn.execute(&format!("drop table {table} purge"))
        .expect("JSON table should be dropped");
    conn.commit().expect("table cleanup should be committed");
}

#[test]
fn json_input_supports_owned_borrowed_named_and_null_values() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_input");
    let owned = yason(r#"{"kind":"owned","value":1}"#);
    let borrowed = yason(r#"{"kind":"borrowed","value":2}"#);
    let named_value = yason(r#"{"kind":"named","value":3}"#);

    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(1_i32), input(owned.clone())],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(2_i32), input(&*borrowed)],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(3_i32), input(None::<YasonBuf>)],
    )
    .unwrap();
    conn.execute_named_with(
        &format!("insert into {table} (id, value) values (:id, :value)"),
        [named(c"id", input(4_i32)), named(c"value", input(Some(&named_value)))],
    )
    .unwrap();
    conn.commit().unwrap();

    let mut rows = conn
        .query(&format!("select id, value from {table} order by id"))
        .unwrap();
    for (id, expected) in [(1_i32, &owned), (2_i32, &borrowed)] {
        let row = rows.fetch().unwrap().unwrap();
        assert_eq!(row.get::<i32>(0).unwrap(), id);
        assert!(
            row.get::<Option<YasonBuf>>(1)
                .unwrap()
                .unwrap()
                .equals(expected)
                .unwrap()
        );
    }
    let row = rows.fetch().unwrap().unwrap();
    assert_eq!(row.get::<i32>(0).unwrap(), 3);
    assert_eq!(row.get::<Option<YasonBuf>>(1).unwrap(), None);
    let row = rows.fetch().unwrap().unwrap();
    assert_eq!(row.get::<i32>(0).unwrap(), 4);
    assert!(row.get::<YasonBuf>(1).unwrap().equals(&named_value).unwrap());
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_input_supports_large_values_and_clob_sources() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_input_large");
    let large_text = format!(r#"{{"kind":"large","value":"{}"}}"#, "x".repeat(64 * 1024));
    let large = yason(&large_text);
    assert!(large.as_bytes().len() >= 64 * 1024);

    let mut source = conn.temporary_clob().unwrap();
    source.append(&large_text).unwrap();
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(1_i32), input(&large)],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(2_i32), input(&source)],
    )
    .unwrap();
    conn.commit().unwrap();
    source.finish().unwrap();

    let mut rows = conn.query(&format!("select value from {table} order by id")).unwrap();
    for _ in 0..2 {
        assert!(
            rows.fetch()
                .unwrap()
                .unwrap()
                .get::<YasonBuf>(0)
                .unwrap()
                .equals(&large)
                .unwrap()
        );
    }
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_query_decodes_metadata_borrowed_owned_and_nullable_values() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_query");
    let first = yason(r#"{"kind":"first"}"#);
    let second = yason(r#"{"kind":"second"}"#);
    for (id, value) in [(1_i32, &first), (2_i32, &second)] {
        conn.execute_with(
            &format!("insert into {table} (id, value) values (?, ?)"),
            [input(id), input(value)],
        )
        .unwrap();
    }
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(3_i32), input(None::<YasonBuf>)],
    )
    .unwrap();
    conn.commit().unwrap();

    let mut rows = conn
        .query(&format!("select id, value from {table} order by id"))
        .unwrap();
    assert_eq!(rows.columns()[1].data_type_info(), DataTypeInfo::Json);
    assert_eq!(rows.columns()[1].data_type_info().data_type(), DataType::Json);

    let row = rows.fetch().unwrap().unwrap();
    assert_eq!(row.get::<i32>(0).unwrap(), 1);
    assert!(row.get::<&Yason>(1).unwrap().equals(&first).unwrap());
    assert!(row.get::<YasonBuf>(1).unwrap().equals(&first).unwrap());
    assert!(row.get::<Option<&Yason>>(1).unwrap().unwrap().equals(&first).unwrap());

    let row = rows.fetch().unwrap().unwrap();
    assert_eq!(row.get::<Option<YasonBuf>>(1).unwrap().unwrap(), second);

    let row = rows.fetch().unwrap().unwrap();
    assert_eq!(row.get::<Option<YasonBuf>>(1).unwrap(), None);
    assert_eq!(row.get::<Option<&Yason>>(1).unwrap(), None);
    assert!(matches!(row.get::<YasonBuf>(1), Err(Error::NullValue { index: 1 })));
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_query_distinguishes_json_literal_null_from_sql_null() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_null");
    let json_null = yason("null");
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(1_i32), input(&json_null)],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(2_i32), input(None::<YasonBuf>)],
    )
    .unwrap();
    conn.commit().unwrap();

    let mut rows = conn.query(&format!("select value from {table} order by id")).unwrap();
    let row = rows.fetch().unwrap().unwrap();
    assert!(
        row.get::<Option<YasonBuf>>(0)
            .unwrap()
            .unwrap()
            .equals(&json_null)
            .unwrap()
    );
    let row = rows.fetch().unwrap().unwrap();
    assert_eq!(row.get::<Option<YasonBuf>>(0).unwrap(), None);
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_query_supports_complex_unicode_values() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_complex");
    let value = yason(r#"[true,false,{"text":"中\n文","number":-123.45,"empty":[]}]"#);
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(1_i32), input(&value)],
    )
    .unwrap();
    conn.commit().unwrap();

    let returned = conn
        .query_one_map(&format!("select value from {table}"), |row| row.get::<YasonBuf>(0))
        .unwrap();
    assert!(returned.equals(&value).unwrap());
    drop_table(&conn, &table);
}

#[test]
fn json_query_rebinds_large_values_across_rows_and_allows_repeated_reads() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_rebind");
    let small = yason(r#"{"kind":"small"}"#);
    let large = yason(format!(r#"{{"kind":"large","value":"{}"}}"#, "x".repeat(70 * 1024)));
    for (id, value) in [(1_i32, &small), (2_i32, &large), (3_i32, &small)] {
        conn.execute_with(
            &format!("insert into {table} (id, value) values (?, ?)"),
            [input(id), input(value)],
        )
        .unwrap();
    }
    conn.commit().unwrap();

    let mut rows = conn.query(&format!("select value from {table} order by id")).unwrap();
    let row = rows.fetch().unwrap().unwrap();
    assert!(row.get::<&Yason>(0).unwrap().equals(&small).unwrap());
    assert!(row.get::<&Yason>(0).unwrap().equals(&small).unwrap());
    let row = rows.fetch().unwrap().unwrap();
    assert!(row.get::<YasonBuf>(0).unwrap().equals(&large).unwrap());
    let row = rows.fetch().unwrap().unwrap();
    assert!(row.get::<Option<&Yason>>(0).unwrap().unwrap().equals(&small).unwrap());
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_output_supports_non_nullable_nullable_and_large_values() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_output");
    let value = yason(r#"{"kind":"output","value":1}"#);
    let large = yason(format!(r#"{{"kind":"large","value":"{}"}}"#, "x".repeat(70 * 1024)));
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(1_i32), input(&value)],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?)"),
        [input(2_i32), input(&large)],
    )
    .unwrap();
    conn.commit().unwrap();

    let mut returned = yason("null");
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?) returning value into ?"),
        [input(3_i32), input(&value), output(&mut returned)],
    )
    .unwrap();
    assert!(returned.equals(&value).unwrap());

    let mut nullable = Some(yason("null"));
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?) returning value into ?"),
        [input(4_i32), input(None::<YasonBuf>), output(&mut nullable)],
    )
    .unwrap();
    assert_eq!(nullable, None);

    let mut large_returned = yason("null");
    conn.execute_with(
        &format!("insert into {table} (id, value) values (?, ?) returning value into ?"),
        [input(5_i32), input(&large), output(&mut large_returned)],
    )
    .unwrap();
    assert!(large_returned.equals(&large).unwrap());
    drop_table(&conn, &table);
}

#[test]
fn json_output_rejects_sql_null_for_non_nullable_target() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_output_null");
    let mut returned = yason("null");

    let error = conn
        .execute_with(
            &format!("insert into {table} (id, value) values (?, ?) returning value into ?"),
            [input(1_i32), input(None::<YasonBuf>), output(&mut returned)],
        )
        .expect_err("non-nullable JSON output should reject SQL NULL");
    assert!(matches!(error, Error::InvalidArgument(message) if message.contains("NULL")));
    conn.rollback().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_output_parameter_can_be_reused_after_sql_null_error() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_output_reuse_after_null");
    let value = yason(r#"{"kind":"after-null-error"}"#);
    let mut returned = yason("null");
    let mut stmt = conn
        .prepare(&format!(
            "insert into {table} (id, value) values (?, ?) returning value into ?"
        ))
        .unwrap();

    let error = stmt
        .execute([input(1_i32), input(None::<YasonBuf>), output(&mut returned)])
        .expect_err("non-nullable JSON output should reject SQL NULL");
    assert!(matches!(error, Error::InvalidArgument(message) if message.contains("NULL")));
    conn.rollback().unwrap();

    stmt.execute([input(2_i32), input(&value), output(&mut returned)])
        .unwrap();
    assert!(returned.equals(&value).unwrap());

    drop_table(&conn, &table);
}

#[test]
fn json_columns_rebind_with_lobs_and_skipped_reads() {
    let Some(conn) = setup() else { return };
    let table = test_object_name("json_mixed");
    conn.execute(&format!(
        "create table {table} (id integer, value json, b blob, c clob)"
    ))
    .unwrap();

    let first_json = yason(r#"{"row":1}"#);
    let second_json = yason(r#"{"row":2}"#);
    let third_json = yason(r#"{"row":3}"#);
    conn.execute_with(
        &format!("insert into {table} values (?, ?, hextoraw('0102'), 'first')"),
        [input(1_i32), input(&first_json)],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} values (?, ?, hextoraw('030405'), 'second')"),
        [input(2_i32), input(&second_json)],
    )
    .unwrap();
    conn.execute_with(
        &format!("insert into {table} values (?, ?, hextoraw('06'), 'third')"),
        [input(3_i32), input(&third_json)],
    )
    .unwrap();
    conn.commit().unwrap();

    let mut rows = conn
        .query(&format!("select id, value, b, c from {table} order by id"))
        .unwrap();
    {
        let row = rows.fetch().unwrap().unwrap();
        assert_eq!(row.get::<i32>(0).unwrap(), 1);
        assert!(row.get::<YasonBuf>(1).unwrap().equals(&first_json).unwrap());
        let mut blob = Vec::new();
        row.get::<yashandb::Blob<'_>>(2)
            .unwrap()
            .read_to_end(&mut blob)
            .unwrap();
        assert_eq!(blob, [1, 2]);
        // Leave the CLOB unread so the next fetch exercises both cleanup paths.
    }
    {
        let row = rows.fetch().unwrap().unwrap();
        assert_eq!(row.get::<i32>(0).unwrap(), 2);
        assert!(
            row.get::<Option<&Yason>>(1)
                .unwrap()
                .unwrap()
                .equals(&second_json)
                .unwrap()
        );
        let mut text = String::new();
        row.get::<yashandb::Clob<'_>>(3)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "second");
    }
    {
        let row = rows.fetch().unwrap().unwrap();
        assert_eq!(row.get::<i32>(0).unwrap(), 3);
        assert!(row.get::<&Yason>(1).unwrap().equals(&third_json).unwrap());
        let mut blob = Vec::new();
        row.get::<yashandb::Blob<'_>>(2)
            .unwrap()
            .read_to_end(&mut blob)
            .unwrap();
        assert_eq!(blob, [6]);
    }
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn prepared_json_query_and_output_reuse_statement() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_prepared");
    let first = yason(r#"{"id":1}"#);
    let second = yason(r#"{"id":2}"#);
    for (id, value) in [(1_i32, &first), (2_i32, &second)] {
        conn.execute_with(
            &format!("insert into {table} (id, value) values (?, ?)"),
            [input(id), input(value)],
        )
        .unwrap();
    }
    conn.commit().unwrap();

    let mut query = conn
        .prepare(&format!("select value from {table} where id = ?"))
        .unwrap();
    for (id, expected) in [(1_i32, &first), (2_i32, &second)] {
        let mut rows = query.query([input(id)]).unwrap();
        assert!(
            rows.fetch()
                .unwrap()
                .unwrap()
                .get::<YasonBuf>(0)
                .unwrap()
                .equals(expected)
                .unwrap()
        );
        assert!(rows.fetch().unwrap().is_none());
        rows.finish().unwrap();
    }

    let mut output_value = yason("null");
    let mut output_stmt = conn
        .prepare(&format!(
            "insert into {table} (id, value) values (?, ?) returning value into ?"
        ))
        .unwrap();
    output_stmt
        .execute([input(3_i32), input(&first), output(&mut output_value)])
        .unwrap();
    assert!(output_value.equals(&first).unwrap());
    output_stmt
        .execute([input(4_i32), input(&second), output(&mut output_value)])
        .unwrap();
    assert!(output_value.equals(&second).unwrap());
    drop_table(&conn, &table);
}

#[test]
fn prepared_named_json_query_binds_null_and_value() {
    let Some(conn) = setup() else { return };
    let table = create_json_table(&conn, "json_prepared_named");
    let value = yason(r#"{"kind":"named"}"#);
    conn.execute_named_with(
        &format!("insert into {table} (id, value) values (:id, :value)"),
        [named(c"id", input(1_i32)), named(c"value", input(&value))],
    )
    .unwrap();
    conn.execute_named_with(
        &format!("insert into {table} (id, value) values (:id, :value)"),
        [named(c"id", input(2_i32)), named(c"value", input(None::<YasonBuf>))],
    )
    .unwrap();
    conn.commit().unwrap();

    let mut stmt = conn
        .prepare(&format!("select value from {table} where id = :id"))
        .unwrap();
    let mut rows = stmt.query_named([named(c"id", input(1_i32))]).unwrap();
    assert!(
        rows.fetch()
            .unwrap()
            .unwrap()
            .get::<YasonBuf>(0)
            .unwrap()
            .equals(&value)
            .unwrap()
    );
    rows.finish().unwrap();
    let mut rows = stmt.query_named([named(c"id", input(2_i32))]).unwrap();
    assert_eq!(rows.fetch().unwrap().unwrap().get::<Option<YasonBuf>>(0).unwrap(), None);
    rows.finish().unwrap();
    drop_table(&conn, &table);
}

#[test]
fn json_query_reports_type_and_index_errors() {
    let Some(conn) = setup() else { return };
    let mut rows = conn.query("select cast('not json' as varchar(16)) from dual").unwrap();
    let row = rows.fetch().unwrap().unwrap();
    assert!(matches!(
        row.get::<YasonBuf>(0),
        Err(Error::ColumnTypeMismatch {
            index: 0,
            actual: DataType::VarChar,
            ..
        })
    ));
    assert!(matches!(
        row.get::<YasonBuf>(1),
        Err(Error::ColumnIndexOutOfBounds { index: 1, .. })
    ));
    rows.finish().unwrap();
}

#[test]
fn yason_parse_rejects_invalid_json() {
    assert!(YasonBuf::parse("{invalid", false).is_err());
}
