//! Integration tests for non-parameterized execution and streaming queries.

mod common;

use std::sync::Mutex;

use common::{LIB_HOME_VAR, conn_credentials, require_library};
use yashandb::{Connection, DataType, DataTypeInfo, Date, Error, IntervalDS, IntervalYM, Number, Time, Timestamp};

static LIB_LOCK: Mutex<()> = Mutex::new(());

fn setup() -> Option<Connection> {
    let _guard = LIB_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if common::lib_path().is_none() {
        eprintln!("skipping: {LIB_HOME_VAR} not set or library missing");
        return None;
    }
    let Some((url, user, password)) = conn_credentials() else {
        eprintln!("skipping: YASDB_URL/YASDB_USER/YASDB_PASSWORD not all set");
        return None;
    };
    require_library();
    Some(Connection::connect(&url, &user, &password).expect("connection should succeed"))
}

#[test]
fn execute_and_query_scalars() {
    let Some(mut conn) = setup() else { return };
    let result = conn.execute("create temporary table rust_query_test (id integer, name varchar(32))");
    if let Err(Error::Database { .. }) = result {
        // Some server configurations do not permit temporary tables. The query
        // below does not require schema changes and still validates the API.
    } else {
        result.expect("temporary table creation should succeed");
        assert_eq!(
            conn.execute("insert into rust_query_test values (1, 'one')")
                .expect("insert should succeed")
                .rows_affected(),
            1
        );
        assert_eq!(
            conn.execute("update rust_query_test set name = 'two' where id = 1")
                .expect("update should succeed")
                .rows_affected(),
            1
        );
        assert_eq!(
            conn.execute("delete from rust_query_test where id = 1")
                .expect("delete should succeed")
                .rows_affected(),
            1
        );
    }

    let mut rows = conn
        .query("select 42 as id, 'hello' as name from dual")
        .expect("query should succeed");
    assert_eq!(rows.columns().len(), 2);
    let row = rows.fetch().expect("fetch should succeed").expect("one row expected");
    assert_eq!(row.column_count(), 2);
    assert_eq!(row.get::<i32>(0).unwrap(), 42);
    assert_eq!(row.get::<String>(row.columns()[1].name()).unwrap(), "hello");
    assert!(matches!(row.get::<i32>("missing"), Err(Error::ColumnNotFound { .. })));
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();

    assert_eq!(
        conn.query_one_map("select 7 from dual", |row| row.get::<i32>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        conn.query_opt_map("select 7 from dual where 1 = 0", |row| row.get::<i32>(0))
            .unwrap(),
        None
    );
}

#[test]
fn query_drop_allows_connection_reuse() {
    let Some(mut conn) = setup() else { return };
    let rows = conn.query("select 1 from dual").expect("query should succeed");
    drop(rows);
    assert_eq!(
        conn.query_one_map("select 2 from dual", |row| row.get::<i32>(0))
            .unwrap(),
        2
    );
}

#[test]
fn query_rust_converted_types() {
    let Some(mut conn) = setup() else { return };
    let value = conn
        .query_one_map("select 123.45 from dual", |row| row.get::<Number>(0))
        .expect("NUMBER should read through Number");
    assert_eq!(value.to_string(), "123.45");

    let date = conn
        .query_one_map("select date '2024-01-02' from dual", |row| row.get::<Date>(0))
        .expect("DATE should read through Date");
    assert_eq!(date.extract().0.extract(), (2024, 1, 2));

    let timestamp = conn
        .query_one_map("select timestamp '2024-01-02 12:34:56' from dual", |row| {
            row.get::<Timestamp>(0)
        })
        .expect("TIMESTAMP should read through Timestamp");
    assert_eq!(timestamp.extract().0.extract(), (2024, 1, 2));

    let ym = conn
        .query_one_map("select interval '2-3' year to month from dual", |row| {
            row.get::<IntervalYM>(0)
        })
        .expect("YM interval should read through IntervalYM");
    assert_eq!(ym.extract().1..=ym.extract().2, 2..=3);

    let ds = conn
        .query_one_map("select interval '2 03:04:05' day to second from dual", |row| {
            row.get::<IntervalDS>(0)
        })
        .expect("DS interval should read through IntervalDS");
    assert_eq!(ds.extract().1..=ds.extract().4, 2..=5);
}

#[test]
fn query_all_supported_scalar_types() {
    let Some(mut conn) = setup() else { return };
    let mut rows = conn
        .query(
            "select cast(1 as boolean) as b, cast(-8 as tinyint) as ti, cast(-16 as smallint) as si, \
             cast(-32 as integer) as i, cast(-64 as bigint) as bi, cast(1.5 as float) as f, \
             cast(2.5 as double) as d, cast('text' as char(8)) as c, cast('ntext' as nchar(8)) as nc, \
             cast('vtext' as varchar(8)) as vc, cast('nvtext' as nvarchar(8)) as nvc, \
             hextoraw('CAFE') as bin from dual",
        )
        .expect("supported scalar query should succeed");
    let row = rows.fetch().unwrap().expect("one row expected");
    assert!(row.get::<bool>(0).unwrap());
    assert_eq!(row.get::<i8>(1).unwrap(), -8);
    assert_eq!(row.get::<i16>(2).unwrap(), -16);
    assert_eq!(row.get::<i32>(3).unwrap(), -32);
    assert_eq!(row.get::<i64>(4).unwrap(), -64);
    assert_eq!(row.get::<f32>(5).unwrap(), 1.5);
    assert_eq!(row.get::<f64>(6).unwrap(), 2.5);
    let character_types = [
        row.columns()[7].data_type_info(),
        row.columns()[8].data_type_info(),
        row.columns()[9].data_type_info(),
        row.columns()[10].data_type_info(),
    ];
    assert!(matches!(character_types[0], DataTypeInfo::Char { char_size: 8, size } if size >= 8));
    assert!(matches!(character_types[1], DataTypeInfo::NChar { char_size: 8, size } if size >= 8));
    assert!(matches!(character_types[2], DataTypeInfo::VarChar { char_size: 8, size } if size >= 8));
    assert!(matches!(character_types[3], DataTypeInfo::NVarChar { char_size: 8, size } if size >= 8));
    assert_eq!(row.get::<String>(7).unwrap().trim(), "text");
    assert_eq!(row.get::<&str>(8).unwrap().trim(), "ntext");
    assert_eq!(row.get::<Option<String>>(9).unwrap().as_deref(), Some("vtext"));
    assert_eq!(row.get::<Option<&str>>(10).unwrap().map(str::trim), Some("nvtext"));
    assert!(matches!(row.columns()[11].data_type_info(), DataTypeInfo::Binary { size } if size >= 2));
    assert_eq!(row.get::<Vec<u8>>(11).unwrap(), [0xca, 0xfe]);
    assert_eq!(row.get::<&[u8]>(11).unwrap(), [0xca, 0xfe]);
    assert!(rows.fetch().unwrap().is_none());
}

#[test]
fn query_time_null_metadata_and_type_errors() {
    let Some(mut conn) = setup() else { return };
    let mut rows = conn
        .query(
            "select cast('12:34:56.123' as time) as tm, cast(null as integer) as nullable_int, \
             cast(123.45 as number(10,2)) as amount, cast(null as varchar(8)) as nullable_text from dual",
        )
        .expect("time and null query should succeed");
    let row = rows.fetch().unwrap().expect("one row expected");
    let time = row.get::<Time>(0).unwrap();
    assert_eq!(time.extract(), (12, 34, 56, 123_000));
    assert_eq!(row.get::<Option<i32>>(1).unwrap(), None);
    assert!(matches!(row.get::<i32>(1), Err(Error::NullValue { index: 1 })));
    assert_eq!(row.get::<Option<String>>(3).unwrap(), None);
    assert!(matches!(
        row.get::<i32>(0),
        Err(Error::ColumnTypeMismatch { index: 0, .. })
    ));
    assert!(matches!(
        row.get::<f64>(2),
        Err(Error::ColumnTypeMismatch { index: 2, .. })
    ));

    assert_eq!(row.columns()[0].name(), "TM");
    assert_eq!(row.columns()[0].data_type_info(), DataTypeInfo::Time);
    assert_eq!(row.columns()[0].data_type_info().data_type(), DataType::Time);
    assert!(row.columns()[1].nullable());
    assert_eq!(
        row.columns()[2].data_type_info(),
        DataTypeInfo::Number {
            precision: 10,
            scale: 2
        }
    );
    assert_eq!(
        row.columns()[3].data_type_info(),
        DataTypeInfo::VarChar { size: 8, char_size: 8 }
    );
    assert!(matches!(
        row.get::<i32>(99),
        Err(Error::ColumnIndexOutOfBounds {
            index: 99,
            column_count: 4
        })
    ));
    assert!(matches!(row.get::<i32>("tm"), Err(Error::ColumnNotFound { .. })));
    assert!(rows.fetch().unwrap().is_none());
}

#[test]
fn query_one_and_optional_cardinality_contracts() {
    let Some(mut conn) = setup() else { return };
    assert!(matches!(
        conn.query_one_map("select 1 from dual where 1 = 0", |_| Ok(()))
            .unwrap_err(),
        Error::RowNotFound
    ));
    assert!(matches!(
        conn.query_one_map("select 1 from dual union all select 2 from dual", |_| Ok(()))
            .unwrap_err(),
        Error::TooManyRows
    ));
    assert_eq!(
        conn.query_opt_map("select 7 from dual", |row| row.get::<i32>(0))
            .unwrap(),
        Some(7)
    );
    assert!(matches!(
        conn.query_opt_map("select 1 from dual union all select 2 from dual", |_| Ok(()))
            .unwrap_err(),
        Error::TooManyRows
    ));
    assert!(matches!(
        conn.query_one_map("select 1 from dual", |_| Err::<(), _>(Error::Internal("mapper failed".into())))
            .unwrap_err(),
        Error::Internal(message) if message == "mapper failed"
    ));
}

#[test]
fn query_nulls_for_all_supported_types() {
    let Some(mut conn) = setup() else { return };
    let mut rows = conn
        .query(
            "select cast(null as boolean), cast(null as tinyint), cast(null as smallint), \
             cast(null as integer), cast(null as bigint), cast(null as float), cast(null as double), \
             cast(null as number), cast(null as date), cast(null as time), cast(null as timestamp), \
             cast(null as interval year to month), cast(null as interval day to second), cast(null as char(8)), \
             cast(null as nchar(8)), cast(null as varchar(8)), cast(null as nvarchar(8)), cast(null as binary(8)) from dual",
        )
        .expect("null query should succeed");
    let row = rows.fetch().unwrap().expect("one row expected");
    assert_eq!(row.get::<Option<bool>>(0).unwrap(), None);
    assert_eq!(row.get::<Option<i8>>(1).unwrap(), None);
    assert_eq!(row.get::<Option<i16>>(2).unwrap(), None);
    assert_eq!(row.get::<Option<i32>>(3).unwrap(), None);
    assert_eq!(row.get::<Option<i64>>(4).unwrap(), None);
    assert_eq!(row.get::<Option<f32>>(5).unwrap(), None);
    assert_eq!(row.get::<Option<f64>>(6).unwrap(), None);
    assert_eq!(row.get::<Option<Number>>(7).unwrap(), None);
    assert_eq!(row.get::<Option<Date>>(8).unwrap(), None);
    assert_eq!(row.get::<Option<Time>>(9).unwrap(), None);
    assert_eq!(row.get::<Option<Timestamp>>(10).unwrap(), None);
    assert_eq!(row.get::<Option<IntervalYM>>(11).unwrap(), None);
    assert_eq!(row.get::<Option<IntervalDS>>(12).unwrap(), None);
    assert_eq!(row.get::<Option<String>>(13).unwrap(), None);
    assert_eq!(row.get::<Option<&str>>(14).unwrap(), None);
    assert_eq!(row.get::<Option<String>>(15).unwrap(), None);
    assert_eq!(row.get::<Option<&str>>(16).unwrap(), None);
    assert_eq!(row.get::<Option<Vec<u8>>>(17).unwrap(), None);
    assert_eq!(row.get::<Option<&[u8]>>(17).unwrap(), None);
}

#[test]
fn query_streams_rows_and_defers_unsupported_type_errors() {
    let Some(mut conn) = setup() else { return };
    let mut rows = conn
        .query("select 1 as id from dual union all select 2 from dual union all select 3 from dual")
        .expect("multi-row query should succeed");
    for expected in 1..=3 {
        assert_eq!(rows.fetch().unwrap().unwrap().get::<i32>(0).unwrap(), expected);
    }
    assert!(rows.fetch().unwrap().is_none());
    assert!(rows.fetch().unwrap().is_none());
    rows.finish().unwrap();

    let mut rows = conn
        .query(
            "select cast('2024-01-02 12:34:56' as timestamp with local time zone) as ltz, \
             cast('2024-01-02 12:34:56 +00:00' as timestamp with time zone) as tz from dual",
        )
        .expect("time zone timestamp query should expose metadata");
    assert_eq!(rows.columns()[0].data_type_info(), DataTypeInfo::TimestampLtz);
    assert_eq!(rows.columns()[1].data_type_info(), DataTypeInfo::TimestampTz);
    let row = rows.fetch().unwrap().expect("one row expected");
    assert!(matches!(
        row.get::<Timestamp>(0),
        Err(Error::UnsupportedColumnType {
            index: 0,
            data_type: DataType::TimestampLtz,
            ..
        })
    ));
    assert!(matches!(
        row.get::<Timestamp>(1),
        Err(Error::UnsupportedColumnType {
            index: 1,
            data_type: DataType::TimestampTz,
            ..
        })
    ));
}
