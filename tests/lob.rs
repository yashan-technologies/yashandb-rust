//! Integration tests for connection-bound LOBs.

mod common;

use std::sync::Mutex;

use common::{conn_credentials, require_library};
use yashandb::{Connection, Error, input, output};

static LIB_LOCK: Mutex<()> = Mutex::new(());

fn connection() -> Option<Connection> {
    let (url, user, password) = conn_credentials()?;
    require_library();
    Some(Connection::connect(&url, &user, &password).expect("database connection failed"))
}

#[test]
fn temporary_blob_supports_positioned_io() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };
    let mut blob = conn.temporary_blob().expect("temporary BLOB");
    assert_eq!(blob.len().unwrap(), 0);
    assert_eq!(blob.append(&[1, 2, 3, 4]).unwrap(), 4);
    assert_eq!(blob.write_at(2, &[9, 8]).unwrap(), 2);
    let mut value = [0; 4];
    assert_eq!(blob.read_at(1, &mut value).unwrap(), 4);
    assert_eq!(value, [1, 9, 8, 4]);
    blob.truncate(3).unwrap();
    assert_eq!(blob.len().unwrap(), 3);

    blob.write_at(2, &[7, 6]).unwrap();
    let mut all = vec![0];
    blob.read_to_end(&mut all).unwrap();
    assert_eq!(all, [0, 1, 7, 6]);
}

#[test]
fn read_to_end_reads_multiple_blob_chunks() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };
    let mut blob = conn.temporary_blob().expect("temporary BLOB");
    let chunk_size = blob.chunk_size().expect("BLOB chunk size").max(1);
    let input = vec![0xA5; chunk_size.saturating_mul(2).saturating_add(1)];
    blob.append(&input).expect("append BLOB data");

    let mut output = Vec::new();
    blob.read_to_end(&mut output).expect("read BLOB data");
    assert_eq!(output, input);
}

#[test]
fn temporary_clob_uses_character_offsets_and_utf8() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };
    let mut clob = conn.temporary_clob().expect("temporary CLOB");
    assert_eq!(clob.append("ABC").unwrap(), 3);
    let mut value = String::from("prefix");
    assert_eq!(clob.read_at(1, 3, &mut value).unwrap(), 3);
    assert_eq!(value, "prefixABC");
    assert_eq!(clob.write_at(2, "中").unwrap(), 1);
    value.clear();
    clob.read_to_string(&mut value).unwrap();
    assert_eq!(value, "A中C");
    assert!(matches!(clob.read_at(0, 1, &mut value), Err(Error::InvalidArgument(_))));
}

#[test]
fn read_to_string_reads_multiple_clob_chunks() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };
    let mut clob = conn.temporary_clob().expect("temporary CLOB");
    let chunk_size = clob.chunk_size().expect("CLOB chunk size").max(1);
    let input = "x".repeat(chunk_size.saturating_mul(2).saturating_add(1));
    clob.append(&input).expect("append CLOB data");

    let mut output = String::new();
    clob.read_to_string(&mut output).expect("read CLOB data");
    assert_eq!(output, input);
}

#[test]
fn multiple_lobs_bind_as_parameters() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };
    let mut blob = conn.temporary_blob().unwrap();
    let mut clob = conn.temporary_clob().unwrap();
    blob.append(&[0xCA, 0xFE]).unwrap();
    clob.append("payload").unwrap();
    conn.execute("begin execute immediate 'create table rust_lob_bind_test (b blob, c clob)'; exception when others then null; end;").unwrap();
    conn.execute_with(
        "insert into rust_lob_bind_test (b, c) values (?, ?)",
        [input(&blob), input(&clob)],
    )
    .unwrap();
    conn.execute_with(
        "insert into rust_lob_bind_test (b, c) values (?, ?)",
        [input(Some(&blob)), input(Some(&clob))],
    )
    .unwrap();
    conn.execute_with(
        "insert into rust_lob_bind_test (b, c) values (?, ?)",
        [input(None::<&yashandb::Blob<'_>>), input(None::<&yashandb::Clob<'_>>)],
    )
    .unwrap();
    conn.commit().unwrap();
    assert!(blob.is_temporary());
    assert!(clob.is_temporary());
    blob.finish().unwrap();
    clob.finish().unwrap();
    conn.execute("delete from rust_lob_bind_test").unwrap();
    conn.commit().unwrap();
    conn.execute("drop table rust_lob_bind_test").ok();
    conn.commit().ok();
}

#[test]
fn query_lobs_can_be_read_before_result_set_finish() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };
    conn.execute("begin execute immediate 'drop table rust_lob_query_test'; exception when others then null; end;")
        .unwrap();
    conn.execute("create table rust_lob_query_test (id integer, b blob, c clob, n nclob)")
        .unwrap();
    conn.commit().unwrap();
    conn.execute("delete from rust_lob_query_test").unwrap();
    conn.commit().unwrap();

    let mut blob = conn.temporary_blob().unwrap();
    let mut clob = conn.temporary_clob().unwrap();
    blob.append(&[1, 2, 3]).unwrap();
    let first_text = "查询";
    assert_ne!(first_text.len(), first_text.encode_utf16().count());
    clob.append(first_text).unwrap();
    conn.execute_with(
        "insert into rust_lob_query_test (id, b, c, n) values (?, ?, ?, ?)",
        [input(1_i32), input(&blob), input(&clob), input(&clob)],
    )
    .unwrap();
    conn.execute_with(
        "insert into rust_lob_query_test (id, b, c, n) values (?, ?, ?, ?)",
        [
            input(2_i32),
            input(None::<&yashandb::Blob<'_>>),
            input(None::<&yashandb::Clob<'_>>),
            input(None::<&yashandb::Clob<'_>>),
        ],
    )
    .unwrap();
    blob.truncate(0).unwrap();
    blob.append(&[4, 5, 6]).unwrap();
    clob.truncate(0).unwrap();
    let second_text = "第二行";
    assert_ne!(second_text.len(), second_text.encode_utf16().count());
    clob.append(second_text).unwrap();
    conn.execute_with(
        "insert into rust_lob_query_test (id, b, c, n) values (?, ?, ?, ?)",
        [input(3_i32), input(&blob), input(&clob), input(&clob)],
    )
    .unwrap();
    conn.commit().unwrap();
    drop(blob);
    drop(clob);

    let mut rows = conn
        .query("select id, b, c, n from rust_lob_query_test order by id")
        .unwrap();
    {
        let row = rows.fetch().unwrap().unwrap();
        let mut blob = row.get::<yashandb::Blob<'_>>(1).unwrap();
        assert!(matches!(
            row.get::<yashandb::Blob<'_>>(1),
            Err(Error::ColumnValueTransferred { index: 1 })
        ));
        let mut blob_value = Vec::new();
        blob.read_to_end(&mut blob_value).unwrap();
        assert_eq!(blob_value, [1, 2, 3]);
        let mut clob_value = String::new();
        let mut clob = row.get::<yashandb::Clob<'_>>(2).unwrap();
        clob.read_to_string(&mut clob_value).unwrap();
        assert_eq!(clob_value, "查询");
        let mut nclob = row.get::<yashandb::Clob<'_>>(3).unwrap();
        clob_value.clear();
        nclob.read_to_string(&mut clob_value).unwrap();
        assert_eq!(clob_value, "查询");
    }
    {
        let row = rows.fetch().unwrap().unwrap();
        assert!(row.get::<Option<yashandb::Blob<'_>>>(1).unwrap().is_none());
        assert!(row.get::<Option<yashandb::Clob<'_>>>(2).unwrap().is_none());
        assert!(row.get::<Option<yashandb::Clob<'_>>>(3).unwrap().is_none());
    }
    {
        let row = rows.fetch().unwrap().unwrap();
        let mut blob = row.get::<yashandb::Blob<'_>>(1).unwrap();
        let mut blob_value = Vec::new();
        blob.read_to_end(&mut blob_value).unwrap();
        assert_eq!(blob_value, [4, 5, 6]);
        let mut clob = row.get::<yashandb::Clob<'_>>(2).unwrap();
        let mut clob_value = String::new();
        clob.read_to_string(&mut clob_value).unwrap();
        assert_eq!(clob_value, "第二行");
        let mut nclob = row.get::<yashandb::Clob<'_>>(3).unwrap();
        clob_value.clear();
        nclob.read_to_string(&mut clob_value).unwrap();
        assert_eq!(clob_value, "第二行");
    }
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();

    conn.execute("delete from rust_lob_query_test").unwrap();
    conn.commit().unwrap();
    conn.execute("drop table rust_lob_query_test").unwrap();
    conn.commit().unwrap();
}

#[test]
fn query_lobs_survive_later_fetch_and_result_set_finish() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };

    conn.execute("begin execute immediate 'drop table rust_lob_lifetime_test'; exception when others then null; end;")
        .unwrap();
    conn.execute("create table rust_lob_lifetime_test (id integer, b blob, c clob)")
        .unwrap();
    conn.execute("insert into rust_lob_lifetime_test values (1, hextoraw('010203'), '第一行')")
        .unwrap();
    conn.execute("insert into rust_lob_lifetime_test values (2, hextoraw('A0B0C0'), '第二行')")
        .unwrap();
    conn.commit().unwrap();

    let mut rows = conn
        .query("select b, c from rust_lob_lifetime_test order by id")
        .unwrap();
    let mut lobs = Vec::new();
    while let Some(row) = rows.fetch().unwrap() {
        let blob = row.get::<yashandb::Blob<'_>>(0).unwrap();
        let clob = row.get::<yashandb::Clob<'_>>(1).unwrap();
        lobs.push((blob, clob));
    }
    rows.finish().unwrap();

    assert_eq!(lobs.len(), 2);
    for (index, (mut blob, mut clob)) in lobs.into_iter().enumerate() {
        let mut blob_value = Vec::new();
        blob.read_to_end(&mut blob_value).unwrap();
        let expected_blob = if index == 0 { [1, 2, 3] } else { [0xA0, 0xB0, 0xC0] };
        assert_eq!(blob_value, expected_blob);

        let mut clob_value = String::new();
        clob.read_to_string(&mut clob_value).unwrap();
        let expected_clob = if index == 0 { "第一行" } else { "第二行" };
        assert_eq!(clob_value, expected_clob);
    }

    conn.execute("delete from rust_lob_lifetime_test").unwrap();
    conn.commit().unwrap();
    conn.execute("drop table rust_lob_lifetime_test").unwrap();
    conn.commit().unwrap();
}

#[test]
fn lob_output_binds_non_nullable_and_nullable_values() {
    let _lock = LIB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(conn) = connection() else {
        eprintln!("skipping: LOB integration environment is not configured");
        return;
    };

    let mut blob = conn.output_blob().unwrap();
    let mut clob = conn.output_clob().unwrap();
    assert!(!blob.is_temporary());
    assert!(!clob.is_temporary());
    let mut nullable_blob: Option<yashandb::Blob<'_>> = None;
    let mut nullable_clob: Option<yashandb::Clob<'_>> = None;
    conn.execute_named_with(
        "begin :b := hextoraw('6F757470757420626C6F62'); :c := 'output clob'; \
         :nb := hextoraw('6E756C6C61626C6520626C6F62'); :nc := cast(null as clob); end;",
        [
            yashandb::named(c"b", output(&mut blob)),
            yashandb::named(c"c", output(&mut clob)),
            yashandb::named(c"nb", output(&mut nullable_blob)),
            yashandb::named(c"nc", output(&mut nullable_clob)),
        ],
    )
    .unwrap();

    assert!(blob.is_temporary());
    assert!(clob.is_temporary());
    let mut blob_value = Vec::new();
    blob.read_to_end(&mut blob_value).unwrap();
    assert_eq!(blob_value, b"output blob");
    let mut clob_value = String::new();
    clob.read_to_string(&mut clob_value).unwrap();
    assert_eq!(clob_value, "output clob");
    let mut nullable_blob = nullable_blob.unwrap();
    let mut nullable_blob_value = Vec::new();
    nullable_blob.read_to_end(&mut nullable_blob_value).unwrap();
    assert_eq!(nullable_blob_value, b"nullable blob");
    assert!(nullable_clob.is_none());

    blob.finish().unwrap();
    clob.finish().unwrap();
    nullable_blob.finish().unwrap();
}
