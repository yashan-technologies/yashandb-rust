//! Integration tests for prepared statements and parameter binding.

mod common;

use std::sync::Mutex;

use common::{LIB_HOME_VAR, conn_credentials, require_library};
use yashandb::{
    Connection, Date, Error, IntervalDS, IntervalYM, Number, Time, Timestamp, in_out, input, named, output,
};

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
fn prepared_positional_query_reuses_statement_after_finish() {
    let Some(conn) = setup() else { return };
    let mut stmt = conn
        .prepare("select cast(? as bigint) from dual")
        .expect("prepare should succeed");

    for expected in [7_i64, 11_i64] {
        let mut rows = stmt.query([input(expected)]).expect("prepared query should succeed");
        let row = rows.fetch().expect("fetch should succeed").expect("one row expected");
        assert_eq!(row.get::<i64>(0).unwrap(), expected);
        assert!(rows.fetch().unwrap().is_none());
        rows.finish().expect("result cleanup should succeed");
    }
}

#[test]
fn temporary_and_named_prepared_queries_bind_values() {
    let Some(conn) = setup() else { return };
    assert!(matches!(
        conn.query_with(
            "select cast(? as varchar(32)) from dual",
            [input("text"), input(vec![1_u8, 2, 3])]
        ),
        Err(Error::InvalidArgument(_))
    ));

    let mut rows = conn
        .query_named_with(
            "select cast(:value as integer) from dual",
            [named(c"value", input(42_i32))],
        )
        .expect("named prepared query should succeed");
    let row = rows.fetch().expect("fetch should succeed").expect("one row expected");
    assert_eq!(row.get::<i32>(0).unwrap(), 42);
    rows.finish().expect("result cleanup should succeed");
}

#[test]
fn named_binds_require_exact_parameter_count() {
    let Some(conn) = setup() else { return };
    let mut stmt = conn
        .prepare("select cast(:first as integer), cast(:second as integer) from dual")
        .expect("prepare should succeed");

    let error = match stmt.query_named([named(c"first", input(1_i32))]) {
        Ok(_) => panic!("missing named bind should fail before execution"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::InvalidArgument(message) if message.contains("expects 2 parameters, received 1")));
}

#[test]
fn invalid_named_binds_preserve_nullable_variable_output() {
    let Some(conn) = setup() else { return };
    let mut value = Some(String::from("keep"));
    let mut stmt = conn
        .prepare("begin :value := :value; end;")
        .expect("prepare should succeed");

    let error = stmt
        .execute_named([
            named(c"value", output((&mut value, 16))),
            named(c"value", input("duplicate")),
        ])
        .expect_err("duplicate named bind should fail before execution");
    assert!(matches!(error, Error::InvalidArgument(message) if message.contains("duplicate")));
    assert_eq!(value.as_deref(), Some("keep"));
}

#[test]
fn execute_with_binds_insert_returning_sequence_value() {
    let Some(conn) = setup() else { return };

    conn.execute("drop table rust_bind_returning_test purge").ok();
    conn.execute("drop sequence rust_bind_returning_seq").ok();
    conn.execute("create table rust_bind_returning_test (id bigint, name varchar(32))")
        .expect("returning test table should be created");
    conn.execute("create sequence rust_bind_returning_seq start with 100")
        .expect("returning test sequence should be created");

    let mut returned_id = 0_i64;
    let result = conn
        .execute_with(
            "insert into rust_bind_returning_test(id, name) values (rust_bind_returning_seq.nextval, ?) \
             returning id into ?",
            [input("returned"), output(&mut returned_id)],
        )
        .expect("parameterized insert returning should succeed");

    assert_eq!(result.rows_affected(), 1);
    assert_eq!(returned_id, 100);
    assert_eq!(
        conn.query_one_map("select name from rust_bind_returning_test where id = 100", |row| {
            row.get::<String>(0)
        })
        .unwrap(),
        "returned"
    );

    conn.execute("drop table rust_bind_returning_test purge")
        .expect("returning test table should be dropped");
    conn.execute("drop sequence rust_bind_returning_seq")
        .expect("returning test sequence should be dropped");
}

#[test]
fn nullable_fixed_output_binds_write_back_null_and_values() {
    let Some(conn) = setup() else { return };

    let mut b: Option<bool> = None;
    let mut i8_value: Option<i8> = None;
    let mut i16_value: Option<i16> = None;
    let mut i32_value: Option<i32> = None;
    let mut i64_value: Option<i64> = None;
    let mut f32_value: Option<f32> = None;
    let mut f64_value: Option<f64> = None;
    let mut number: Option<Number> = None;
    let mut date: Option<Date> = None;
    let mut time: Option<Time> = None;
    let mut timestamp: Option<Timestamp> = None;
    let mut ym: Option<IntervalYM> = None;
    let mut ds: Option<IntervalDS> = None;
    conn.prepare(
        "begin :b := cast(true as boolean); :i8 := cast(-8 as tinyint); :i16 := cast(-16 as smallint); \
         :i32 := cast(-32 as integer); :i64 := cast(-64 as bigint); :f32 := cast(1.5 as float); \
         :f64 := cast(2.5 as double); :number := cast(123.45 as number); :date := date '2024-01-02'; \
         :time := cast('12:34:56.123' as time); :timestamp := timestamp '2024-01-02 12:34:56'; \
         :ym := interval '2-3' year to month; :ds := interval '2 03:04:05' day to second; end;",
    )
    .unwrap()
    .execute_named([
        named(c"b", output(&mut b)),
        named(c"i8", output(&mut i8_value)),
        named(c"i16", output(&mut i16_value)),
        named(c"i32", output(&mut i32_value)),
        named(c"i64", output(&mut i64_value)),
        named(c"f32", output(&mut f32_value)),
        named(c"f64", output(&mut f64_value)),
        named(c"number", output(&mut number)),
        named(c"date", output(&mut date)),
        named(c"time", output(&mut time)),
        named(c"timestamp", output(&mut timestamp)),
        named(c"ym", output(&mut ym)),
        named(c"ds", output(&mut ds)),
    ])
    .unwrap();
    assert_eq!(b, Some(true));
    assert_eq!(i8_value, Some(-8));
    assert_eq!(i16_value, Some(-16));
    assert_eq!(i32_value, Some(-32));
    assert_eq!(i64_value, Some(-64));
    assert_eq!(f32_value, Some(1.5));
    assert_eq!(f64_value, Some(2.5));
    assert_eq!(number.unwrap().to_string(), "123.45");
    assert_eq!(date.unwrap().extract().0.extract(), (2024, 1, 2));
    assert_eq!(time.unwrap().extract(), (12, 34, 56, 123_000));
    assert_eq!(timestamp.unwrap().extract().0.extract(), (2024, 1, 2));
    assert_eq!(ym.unwrap().extract().1..=ym.unwrap().extract().2, 2..=3);
    assert_eq!(ds.unwrap().extract().1..=ds.unwrap().extract().4, 2..=5);

    let mut b = Some(false);
    let mut i8_value = Some(1_i8);
    let mut i16_value = Some(2_i16);
    let mut i32_value = Some(3_i32);
    let mut i64_value = Some(4_i64);
    let mut f32_value = Some(5.0_f32);
    let mut f64_value = Some(6.0_f64);
    let mut number = Some("7.5".parse::<Number>().unwrap());
    let mut date: Option<Date> = Some(
        conn.query_one_map("select date '2024-01-02' from dual", |row| row.get(0))
            .unwrap(),
    );
    let mut time: Option<Time> = Some(
        conn.query_one_map("select cast('12:34:56' as time) from dual", |row| row.get(0))
            .unwrap(),
    );
    let mut timestamp: Option<Timestamp> = Some(
        conn.query_one_map("select timestamp '2024-01-02 12:34:56' from dual", |row| row.get(0))
            .unwrap(),
    );
    let mut ym: Option<IntervalYM> = Some(
        conn.query_one_map("select interval '2-3' year to month from dual", |row| row.get(0))
            .unwrap(),
    );
    let mut ds: Option<IntervalDS> = Some(
        conn.query_one_map("select interval '2 03:04:05' day to second from dual", |row| row.get(0))
            .unwrap(),
    );
    conn.prepare(
        "begin :b := cast(true as boolean); :i8 := :i8 + 1; :i16 := :i16 + 1; :i32 := :i32 + 1; \
         :i64 := :i64 + 1; :f32 := :f32 + 1; :f64 := :f64 + 1; :number := :number + 1; \
         :date := :date + 1; :time := cast('23:45:01' as time); :timestamp := :timestamp + interval '1' day; \
         :ym := interval '4-5' year to month; :ds := interval '6 07:08:09' day to second; end;",
    )
    .unwrap()
    .execute_named([
        named(c"b", in_out(&mut b)),
        named(c"i8", in_out(&mut i8_value)),
        named(c"i16", in_out(&mut i16_value)),
        named(c"i32", in_out(&mut i32_value)),
        named(c"i64", in_out(&mut i64_value)),
        named(c"f32", in_out(&mut f32_value)),
        named(c"f64", in_out(&mut f64_value)),
        named(c"number", in_out(&mut number)),
        named(c"date", in_out(&mut date)),
        named(c"time", in_out(&mut time)),
        named(c"timestamp", in_out(&mut timestamp)),
        named(c"ym", in_out(&mut ym)),
        named(c"ds", in_out(&mut ds)),
    ])
    .unwrap();
    assert_eq!(b, Some(true));
    assert_eq!(i8_value, Some(2));
    assert_eq!(i16_value, Some(3));
    assert_eq!(i32_value, Some(4));
    assert_eq!(i64_value, Some(5));
    assert_eq!(f32_value, Some(6.0));
    assert_eq!(f64_value, Some(7.0));
    assert_eq!(number.unwrap().to_string(), "8.5");
    assert_eq!(date.unwrap().extract().0.extract(), (2024, 1, 3));
    assert_eq!(time.unwrap().extract(), (23, 45, 1, 0));
    assert_eq!(timestamp.unwrap().extract().0.extract(), (2024, 1, 3));
    assert_eq!(ym.unwrap().extract().1..=ym.unwrap().extract().2, 4..=5);
    assert_eq!(ds.unwrap().extract().1..=ds.unwrap().extract().4, 6..=9);

    let mut value = Some(7_i64);
    conn.prepare("begin :value := cast(null as bigint); end;")
        .unwrap()
        .execute_named([named(c"value", output(&mut value))])
        .unwrap();
    assert_eq!(value, None);

    let mut value = Some(7_i64);
    conn.prepare("begin :value := cast(null as bigint); end;")
        .unwrap()
        .execute_named([named(c"value", in_out(&mut value))])
        .unwrap();
    assert_eq!(value, None);

    let mut value: Option<i64> = None;
    conn.prepare("begin :value := cast(9 as bigint); end;")
        .unwrap()
        .execute_named([named(c"value", in_out(&mut value))])
        .unwrap();
    assert_eq!(value, Some(9));
}

#[test]
fn nullable_variable_output_uses_explicit_capacity() {
    let Some(conn) = setup() else { return };

    let mut value: Option<String> = None;
    conn.prepare("begin :value := 'hello'; end;")
        .expect("prepare should succeed")
        .execute_named([named(c"value", yashandb::output((&mut value, 16)))])
        .expect("text output should succeed");
    assert_eq!(value.as_deref(), Some("hello"));

    let mut value = Some(String::from("old"));
    conn.prepare("begin :value := cast(null as varchar(16)); end;")
        .expect("prepare should succeed")
        .execute_named([named(c"value", yashandb::output((&mut value, 16)))])
        .expect("null text output should succeed");
    assert_eq!(value, None);

    let mut value: Option<String> = None;
    conn.prepare("begin :value := 'hello'; end;")
        .expect("prepare should succeed")
        .execute_named([named(c"value", yashandb::output((&mut value, 16)))])
        .expect("sized text output should succeed");
    assert_eq!(value.as_deref(), Some("hello"));

    let mut value = Some(String::from("old"));
    conn.prepare("begin :value := 'new'; end;")
        .unwrap()
        .execute_named([named(c"value", yashandb::in_out((&mut value, 16)))])
        .unwrap();
    assert_eq!(value.as_deref(), Some("new"));

    let mut value: Option<String> = None;
    conn.prepare("begin :value := 'new'; end;")
        .unwrap()
        .execute_named([named(c"value", yashandb::in_out((&mut value, 16)))])
        .unwrap();
    assert_eq!(value.as_deref(), Some("new"));

    let mut value = Some(String::from("old"));
    conn.prepare("begin :value := cast(null as varchar(16)); end;")
        .unwrap()
        .execute_named([named(c"value", yashandb::in_out((&mut value, 16)))])
        .unwrap();
    assert_eq!(value, None);

    let mut binary: Option<Vec<u8>> = None;
    conn.prepare("begin :value := hextoraw('CAFE'); end;")
        .unwrap()
        .execute_named([named(c"value", yashandb::output((&mut binary, 8)))])
        .unwrap();
    assert_eq!(binary, Some(vec![0xca, 0xfe]));

    let mut binary = Some(vec![0xaa]);
    conn.prepare("begin :value := hextoraw('CAFE'); end;")
        .unwrap()
        .execute_named([named(c"value", yashandb::in_out((&mut binary, 8)))])
        .unwrap();
    assert_eq!(binary, Some(vec![0xca, 0xfe]));

    let mut binary = Some(vec![0xaa]);
    conn.prepare("begin :value := cast(null as binary(8)); end;")
        .unwrap()
        .execute_named([named(c"value", yashandb::in_out((&mut binary, 8)))])
        .unwrap();
    assert_eq!(binary, None);

    let mut text = String::with_capacity(2);
    let error = conn
        .prepare("begin :value := 'hello'; end;")
        .unwrap()
        .execute_named([named(c"value", output(&mut text))])
        .unwrap_err();
    assert!(matches!(error, Error::Database { .. }));

    let mut binary = Vec::with_capacity(1);
    let error = conn
        .prepare("begin :value := hextoraw('CAFE'); end;")
        .unwrap()
        .execute_named([named(c"value", output(&mut binary))])
        .unwrap_err();
    assert!(matches!(error, Error::Database { .. }));

    let mut value = 0_i64;
    let error = conn
        .prepare("begin :value := cast(null as bigint); end;")
        .unwrap()
        .execute_named([named(c"value", output(&mut value))])
        .unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(message) if message.contains("NULL")));
}

#[test]
fn positional_input_binds_all_supported_types() {
    let Some(conn) = setup() else { return };
    let date = conn
        .query_one_map("select date '2024-01-02' from dual", |row| row.get::<Date>(0))
        .unwrap();
    let time = conn
        .query_one_map("select cast('12:34:56.123' as time) from dual", |row| {
            row.get::<Time>(0)
        })
        .unwrap();
    let timestamp = conn
        .query_one_map("select timestamp '2024-01-02 12:34:56' from dual", |row| {
            row.get::<Timestamp>(0)
        })
        .unwrap();
    let ym = conn
        .query_one_map("select interval '2-3' year to month from dual", |row| {
            row.get::<IntervalYM>(0)
        })
        .unwrap();
    let ds = conn
        .query_one_map("select interval '2 03:04:05' day to second from dual", |row| {
            row.get::<IntervalDS>(0)
        })
        .unwrap();
    let number = "123.45".parse::<Number>().unwrap();

    let mut rows = conn
        .query_with(
            "select cast(? as boolean), cast(? as tinyint), cast(? as smallint), cast(? as integer), \
             cast(? as bigint), cast(? as float), cast(? as double), cast(? as number), cast(? as date), \
             cast(? as time), cast(? as timestamp), cast(? as interval year to month), \
             cast(? as interval day to second), cast(? as varchar(32)), cast(? as varchar(32)), \
             cast(? as binary(8)) from dual",
            [
                input(true),
                input(-8_i8),
                input(-16_i16),
                input(-32_i32),
                input(-64_i64),
                input(1.5_f32),
                input(2.5_f64),
                input(number),
                input(date),
                input(time),
                input(timestamp),
                input(ym),
                input(ds),
                input("borrowed"),
                input(String::from("owned")),
                input(vec![0xca_u8, 0xfe]),
            ],
        )
        .unwrap();
    let row = rows.fetch().unwrap().unwrap();
    assert!(row.get::<bool>(0).unwrap());
    assert_eq!(row.get::<i8>(1).unwrap(), -8);
    assert_eq!(row.get::<i16>(2).unwrap(), -16);
    assert_eq!(row.get::<i32>(3).unwrap(), -32);
    assert_eq!(row.get::<i64>(4).unwrap(), -64);
    assert_eq!(row.get::<f32>(5).unwrap(), 1.5);
    assert_eq!(row.get::<f64>(6).unwrap(), 2.5);
    assert_eq!(row.get::<Number>(7).unwrap().to_string(), "123.45");
    assert_eq!(row.get::<Date>(8).unwrap(), date);
    assert_eq!(row.get::<Time>(9).unwrap(), time);
    assert_eq!(row.get::<Timestamp>(10).unwrap(), timestamp);
    assert_eq!(row.get::<IntervalYM>(11).unwrap(), ym);
    assert_eq!(row.get::<IntervalDS>(12).unwrap(), ds);
    assert_eq!(row.get::<String>(13).unwrap(), "borrowed");
    assert_eq!(row.get::<String>(14).unwrap(), "owned");
    assert_eq!(row.get::<Vec<u8>>(15).unwrap(), [0xca, 0xfe]);
    rows.finish().unwrap();
}

#[test]
fn positional_nullable_input_binds_null_for_all_supported_types() {
    let Some(conn) = setup() else { return };
    let mut rows = conn
        .query_with(
            "select cast(? as boolean), cast(? as tinyint), cast(? as smallint), cast(? as integer), \
             cast(? as bigint), cast(? as float), cast(? as double), cast(? as number), cast(? as date), \
             cast(? as time), cast(? as timestamp), cast(? as interval year to month), \
             cast(? as interval day to second), cast(? as varchar(32)), cast(? as binary(8)) from dual",
            [
                input(Option::<bool>::None),
                input(Option::<i8>::None),
                input(Option::<i16>::None),
                input(Option::<i32>::None),
                input(Option::<i64>::None),
                input(Option::<f32>::None),
                input(Option::<f64>::None),
                input(Option::<Number>::None),
                input(Option::<Date>::None),
                input(Option::<Time>::None),
                input(Option::<Timestamp>::None),
                input(Option::<IntervalYM>::None),
                input(Option::<IntervalDS>::None),
                input(Option::<&str>::None),
                input(Option::<&[u8]>::None),
            ],
        )
        .unwrap();
    let row = rows.fetch().unwrap().unwrap();
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
    assert_eq!(row.get::<Option<Vec<u8>>>(14).unwrap(), None);
    rows.finish().unwrap();
}

#[test]
fn output_binds_all_fixed_types() {
    let Some(conn) = setup() else { return };
    let mut b = false;
    let mut i8_value = 0_i8;
    let mut i16_value = 0_i16;
    let mut i32_value = 0_i32;
    let mut i64_value = 0_i64;
    let mut f32_value = 0.0_f32;
    let mut f64_value = 0.0_f64;
    let mut number = Number::default();
    let mut date = Date::MIN;
    let mut time = Time::ZERO;
    let mut timestamp = Timestamp::MIN;
    let mut ym = IntervalYM::ZERO;
    let mut ds = IntervalDS::ZERO;
    let mut text = String::with_capacity(16);
    let mut binary = Vec::with_capacity(4);
    let sql = "begin \
        :b := cast(true as boolean); \
        :i8 := cast(-8 as tinyint); :i16 := cast(-16 as smallint); \
        :i32 := cast(-32 as integer); :i64 := cast(-64 as bigint); \
        :f32 := cast(1.5 as float); :f64 := cast(2.5 as double); \
        :number := cast(123.45 as number); :date := date '2024-01-02'; \
        :time := cast('12:34:56.123' as time); :timestamp := timestamp '2024-01-02 12:34:56'; \
        :ym := interval '2-3' year to month; :ds := interval '2 03:04:05' day to second; \
        :text := 'hello'; :binary := hextoraw('CAFE'); \
    end;";
    conn.prepare(sql)
        .unwrap()
        .execute_named([
            named(c"b", output(&mut b)),
            named(c"i8", output(&mut i8_value)),
            named(c"i16", output(&mut i16_value)),
            named(c"i32", output(&mut i32_value)),
            named(c"i64", output(&mut i64_value)),
            named(c"f32", output(&mut f32_value)),
            named(c"f64", output(&mut f64_value)),
            named(c"number", output(&mut number)),
            named(c"date", output(&mut date)),
            named(c"time", output(&mut time)),
            named(c"timestamp", output(&mut timestamp)),
            named(c"ym", output(&mut ym)),
            named(c"ds", output(&mut ds)),
            named(c"text", output(&mut text)),
            named(c"binary", output(&mut binary)),
        ])
        .unwrap();
    assert!(b);
    assert_eq!(i8_value, -8);
    assert_eq!(i16_value, -16);
    assert_eq!(i32_value, -32);
    assert_eq!(i64_value, -64);
    assert_eq!(f32_value, 1.5);
    assert_eq!(f64_value, 2.5);
    assert_eq!(number.to_string(), "123.45");
    assert_eq!(date.extract().0.extract(), (2024, 1, 2));
    assert_eq!(time.extract(), (12, 34, 56, 123_000));
    assert_eq!(timestamp.extract().0.extract(), (2024, 1, 2));
    assert_eq!(ym.extract().1..=ym.extract().2, 2..=3);
    assert_eq!(ds.extract().1..=ds.extract().4, 2..=5);
    assert_eq!(text, "hello");
    assert_eq!(binary, [0xca, 0xfe]);
}

#[test]
fn in_out_updates_fixed_and_variable_types() {
    let Some(conn) = setup() else { return };
    let mut bool_value = false;
    let mut i8_value = 0_i8;
    let mut i16_value = 0_i16;
    let mut i32_value = 7_i32;
    let mut i64_value = 0_i64;
    let mut f32_value = 0.0_f32;
    let mut f64_value = 0.0_f64;
    let mut number = Number::default();
    let mut date = Date::MIN;
    let mut time = Time::ZERO;
    let mut timestamp = Timestamp::MIN;
    let mut ym = IntervalYM::ZERO;
    let mut ds = IntervalDS::ZERO;
    let mut text = String::with_capacity(16);
    text.push_str("old");
    let mut binary = Vec::with_capacity(4);
    conn.prepare(
        "begin :b := cast(true as boolean); :i8 := cast(-8 as tinyint); \
         :i16 := cast(-16 as smallint); :i32 := :i32 + 5; :i64 := cast(-64 as bigint); \
         :f32 := cast(1.5 as float); :f64 := cast(2.5 as double); :number := cast(123.45 as number); \
         :date := date '2024-01-02'; :time := cast('12:34:56.123' as time); \
         :timestamp := timestamp '2024-01-02 12:34:56'; :ym := interval '2-3' year to month; \
         :ds := interval '2 03:04:05' day to second; :text := 'new'; \
         :binary := hextoraw('CAFE'); end;",
    )
    .unwrap()
    .execute_named([
        named(c"b", in_out(&mut bool_value)),
        named(c"i8", in_out(&mut i8_value)),
        named(c"i16", in_out(&mut i16_value)),
        named(c"i32", in_out(&mut i32_value)),
        named(c"i64", in_out(&mut i64_value)),
        named(c"f32", in_out(&mut f32_value)),
        named(c"f64", in_out(&mut f64_value)),
        named(c"number", in_out(&mut number)),
        named(c"date", in_out(&mut date)),
        named(c"time", in_out(&mut time)),
        named(c"timestamp", in_out(&mut timestamp)),
        named(c"ym", in_out(&mut ym)),
        named(c"ds", in_out(&mut ds)),
        named(c"text", in_out(&mut text)),
        named(c"binary", in_out(&mut binary)),
    ])
    .unwrap();
    assert!(bool_value);
    assert_eq!(i8_value, -8);
    assert_eq!(i16_value, -16);
    assert_eq!(i32_value, 12);
    assert_eq!(i64_value, -64);
    assert_eq!(f32_value, 1.5);
    assert_eq!(f64_value, 2.5);
    assert_eq!(number.to_string(), "123.45");
    assert_eq!(date.extract().0.extract(), (2024, 1, 2));
    assert_eq!(time.extract(), (12, 34, 56, 123_000));
    assert_eq!(timestamp.extract().0.extract(), (2024, 1, 2));
    assert_eq!(ym.extract().1..=ym.extract().2, 2..=3);
    assert_eq!(ds.extract().1..=ds.extract().4, 2..=5);
    assert_eq!(text, "new");
    assert_eq!(binary, [0xca, 0xfe]);
}

#[test]
fn prepared_statement_reuses_execute_and_named_statement() {
    let Some(conn) = setup() else { return };
    let mut stmt = conn.prepare("select cast(? as integer) from dual").unwrap();
    for expected in [1_i32, 2_i32, 3_i32] {
        let mut rows = stmt.query([input(expected)]).unwrap();
        assert_eq!(rows.fetch().unwrap().unwrap().get::<i32>(0).unwrap(), expected);
        rows.finish().unwrap();
    }
    stmt.finish().unwrap();

    let mut stmt = conn.prepare("begin :value := :value + 1; end;").unwrap();
    let mut value = 10_i32;
    for expected in [11_i32, 12_i32, 13_i32] {
        stmt.execute_named([named(c"value", in_out(&mut value))]).unwrap();
        assert_eq!(value, expected);
    }
}
