# YashanDB Rust Driver

An official Rust driver for [YashanDB](https://www.yashandb.com). It provides a
synchronous, blocking connection to a YashanDB instance.

## Features

- **Blocking `Connection`** to a YashanDB instance, with automatic resource
  management.
- **Automatic client library loading** — the YashanDB client library is loaded
  on first use, with optional explicit-path loading.
- **Non-parameterized SQL execution and streaming queries** with typed result
  rows and column metadata.
- **Prepared statements and parameter binding** for positional and named
  parameters, including input, output, and input/output values.
- **Manual and scoped transactions** with explicit commit and rollback APIs.

## MSRV

The minimum supported Rust version is **1.95**.

## Requirements

- A YashanDB client library (`yascli`) installed on the machine. The driver
  finds it automatically from the default search path, or from the per-user
  install location `$HOME/.yashandb/client/lib` (Linux/macOS) or
  `%USERPROFILE%\.yashandb\client\lib` (Windows).
- A reachable YashanDB instance to connect to.

The client library version **23.4.1.100 or later** is the supported baseline.
Connection attributes the baseline supports are treated as infallible (the
underlying call panics internally on failure), so using an older client library
may cause a panic on an unsupported attribute. Only operations that legitimately
fail at runtime (e.g. setting the transaction isolation mid-transaction, or the
baseline-unsupported login timeout) return a `Result`.

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
yashandb = "0.1"
```

Connect to a database:

```rust
use yashandb::Connection;

fn main() -> Result<(), yashandb::Error> {
    let mut conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
    let answer = conn.query_one_map("select 42 from dual", |row| row.get::<i32>(0))?;
    assert_eq!(answer, 42);
    Ok(())
}
```

The client library is loaded automatically on first use. To fail fast at
startup, call `load_library` first:

```rust
use yashandb::{load_library, Connection};

// Auto-discovery: default search path, then the per-user install directory.
fn main() -> Result<(), yashandb::Error> {
    load_library()?;

    let conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
    // ...
    # let _ = conn;
    Ok(())
}
```

To load a client library from a specific path instead, use only
`load_library_with_path` before connecting:

```rust
use yashandb::{load_library_with_path, Connection};

fn main() -> Result<(), yashandb::Error> {
    load_library_with_path("/opt/yashandb/client/lib/yascli.so")?;
    let conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
    // ...
    # let _ = conn;
    Ok(())
}
```

### Interface usage

Use `execute` for SQL without parameters that does not return rows, and `query`
for a streaming `ResultSet`. The result set exclusively borrows the connection
until it is dropped or `finish`ed.

```rust
# use yashandb::{Connection, Error};
# fn example(conn: &mut Connection) -> Result<(), Error> {
conn.execute("create table users (id integer, name varchar(64))")?;

let mut rows = conn.query("select id, name from users")?;
while let Some(row) = rows.fetch()? {
    let id: i32 = row.get(0)?;
    let name: Option<String> = row.get("NAME")?;
    # let _ = (id, name);
}
rows.finish()?; // Optional, but reports a native release failure.
# Ok(())
# }
```

`Row::get` accepts a zero-based `usize` index or exact database-reported column
name. Wrap a target in `Option<T>` to read a database `NULL`. The supported
query result mapping is:

| SQL type | Rust type |
|---|---|
| `BOOL` | `bool` / `Option<bool>` |
| `TINYINT` | `i8` / `Option<i8>` |
| `SMALLINT` | `i16` / `Option<i16>` |
| `INTEGER` | `i32` / `Option<i32>` |
| `BIGINT` | `i64` / `Option<i64>` |
| `FLOAT` | `f32` / `Option<f32>` |
| `DOUBLE` | `f64` / `Option<f64>` |
| `NUMBER` | `Number` / `Option<Number>` |
| `DATE` | `Date` / `Option<Date>` |
| `SHORTTIME` | `Time` / `Option<Time>` |
| `TIMESTAMP` | `Timestamp` / `Option<Timestamp>` |
| `INTERVAL YEAR TO MONTH` | `IntervalYM` / `Option<IntervalYM>` |
| `INTERVAL DAY TO SECOND` | `IntervalDS` / `Option<IntervalDS>` |
| `CHAR`, `NCHAR`, `VARCHAR`, `NVARCHAR` | `String`, `&str`, or their `Option<T>` forms |
| `BINARY` | `Vec<u8>`, `&[u8]`, or their `Option<T>` forms |

`TIMESTAMP WITH [LOCAL] TIME ZONE` and unrecognized types remain visible in
metadata but return an error only if that column is read.

Use `execute_with` and `query_with` for parameterized SQL that is executed once.
Construct values with `input`, `output`, and `in_out`.

```rust
use yashandb::{Connection, Error, input};

fn insert_user(conn: &mut Connection, id: i64, name: &str) -> Result<(), Error> {
    conn.execute_with(
        "insert into users(id, name) values (?, ?)",
        [input(id), input(name)],
    )?;
    Ok(())
}
```

For repeated execution, use `prepare` and call `Statement::execute` or
`Statement::query` on the returned statement:

```rust
use yashandb::{Connection, Error, input};

fn insert_users(conn: &mut Connection, users: &[(i64, &str)]) -> Result<(), Error> {
    let mut stmt = conn.prepare("insert into users(id, name) values (?, ?)")?;
    for &(id, name) in users {
        stmt.execute([input(id), input(name)])?;
    }
    Ok(())
}
```

Parameter lists are passed as arrays, slices, or `Vec<BindParam>` values.

`Option<T>` input values bind SQL `NULL`. Use a type annotation when the type
cannot be inferred, for example `input(Option::<i64>::None)`.

The supported parameter mappings are:

| Rust type | YashanDB/YACLI type |
|---|---|
| `bool` | `BOOL` |
| `i8` | `TINYINT` |
| `i16` | `SMALLINT` |
| `i32` | `INTEGER` |
| `i64` | `BIGINT` |
| `f32` | `FLOAT` |
| `f64` | `DOUBLE` |
| `Number` | `NUMBER` |
| `Date` | `DATE` |
| `Time` | `SHORTTIME` |
| `Timestamp` | `TIMESTAMP` |
| `IntervalYM` | `INTERVAL YEAR TO MONTH` |
| `IntervalDS` | `INTERVAL DAY TO SECOND` |
| `&str`, `String` | `VARCHAR` |
| `&[u8]`, `Vec<u8>` | `BINARY` |

The corresponding `Option<T>` forms keep the same database type and add SQL
`NULL` handling. The complete accepted forms for `input`, `output`, and
`in_out` are documented on those functions.

Named parameters use a NUL-terminated `CString` or `&CStr`. Pass the name
without the SQL placeholder prefix: `value` corresponds to `:value` in SQL.

```rust
use std::ffi::CString;
use yashandb::{Connection, Error, input, named};

fn call_procedure(conn: &mut Connection, id: i64) -> Result<(), Error> {
    let name = CString::new("id").map_err(|_| Error::InvalidArgument("invalid parameter name".into()))?;
    conn.execute_named_with(
        "begin process_user(:id); end;",
        [named(name, input(id))],
    )?;
    Ok(())
}
```

`output` and `in_out` values are written back to mutable Rust targets after
execution. `output` only receives the database value; `in_out` also sends the
target's initial value to the database:

```rust
use std::ffi::CString;
use yashandb::{Connection, Error, named, output};

fn read_output(conn: &mut Connection) -> Result<(Option<i64>, String), Error> {
    let mut value: Option<i64> = None;
    let name = CString::new("value").map_err(|_| Error::InvalidArgument("invalid parameter name".into()))?;
    conn.prepare("begin :value := cast(42 as bigint); end;")?
        .execute_named([named(name, output(&mut value))])?;

    let mut text = String::with_capacity(128);
    let name = CString::new("text").map_err(|_| Error::InvalidArgument("invalid parameter name".into()))?;
    conn.prepare("begin :text := 'hello'; end;")?
        .execute_named([named(name, output(&mut text))])?;
    Ok((value, text))
}
```

For an input/output parameter:

```rust
let mut value = 7_i64;
stmt.execute([in_out(&mut value)])?;
```

For nullable variable-size output, provide a minimum buffer capacity. An
existing target allocation with greater capacity may be reused:

```rust
let mut value: Option<String> = None;
stmt.execute_named([named(
    c"value".as_c_str(),
    output((&mut value, 128)),
)])?;
```

`String` and `Vec<u8>` use their existing capacity as the output limit. Output
that exceeds the capacity returns an error rather than being truncated. If an
execution returns an error, output and input/output targets may already contain
data written by the client; their updates are not atomic. A prepared query
borrows the statement until its `ResultSet` is finished or dropped; finish the
result set before executing the statement again.

### Transactions

New connections use manual commit by default. Call `commit` to make current
connection work visible, or `rollback` to discard it; neither method changes
the auto-commit setting.

```rust
use yashandb::{Connection, Error};

fn create_user(conn: &mut Connection) -> Result<(), Error> {
    conn.execute("insert into users(id, name) values (1, 'Alice')")?;
    conn.commit()?;
    Ok(())
}
```

For a scoped transaction on an auto-commit connection, enable auto-commit and
use `transaction`. The guard temporarily disables auto-commit and restores it
after `commit`, `rollback`, or drop. An unfinished guard attempts to roll back;
call `rollback` explicitly when the rollback error must be handled.

```rust
use yashandb::{Connection, Error};

fn transfer(conn: &mut Connection) -> Result<(), Error> {
    conn.set_auto_commit(true);
    let mut tx = conn.transaction();
    tx.execute("update accounts set balance = balance - 10 where id = 1")?;
    tx.execute("update accounts set balance = balance + 10 where id = 2")?;
    tx.commit()
}
```

With manual commit enabled, `transaction` guards the connection's current
transaction, including work performed before the guard was created. It does not
send `BEGIN`, create an independent server transaction, or support nesting. A
result set or statement created from a transaction must be finished or dropped
before committing or rolling back. The driver permits transaction-control SQL;
when an application executes it directly, the application is responsible for
maintaining transaction state consistent with the guard.

### Connection URL formats

The `url` argument passed to `Connection::connect` supports several formats:

- **Single address:** `host:port`
- **Multiple addresses:** `serverType:host:port,host:port,host:port`
  (addresses separated by `,`)
- **Multiple address groups:** `serverType:host:port,host:port;host:port,host:port`
  (groups separated by `;`, addresses within a group separated by `,`)

`host` may be an IPv4 address, an IPv6 address, or a domain name. `port`
defaults to `1688` when omitted.

`serverType` is optional and selects the connection strategy:

| `serverType` | Behavior |
|---|---|
| `primary` (default) | Connect addresses in order, keep the first connection to a primary node |
| `standby` | Connect addresses in order, keep the first connection to a standby node |
| `loadBalance` | Shuffle addresses, pick the node with the fewest sessions |
| `primaryLoadBalance` | Shuffle addresses, pick the primary node with the fewest sessions |
| `standbyLoadBalance` | Shuffle addresses, pick the standby node with the fewest sessions |

### Error handling

All fallible operations return `Result<_, yashandb::Error>`. The error type
implements `std::error::Error` and is `#[non_exhaustive]`, so it may grow new
variants in future releases.

## Thread safety

- `Connection` is `Send`: it can be moved to another thread, e.g. sent over a
  channel to a worker thread.
- `Connection` is `!Sync`: it must not be shared across threads.

The client library is a process-global singleton loaded exactly once. The first
load wins; loading a *different* library afterwards returns
`Error::ClientLibrary`.

## Testing

The integration tests require a real YashanDB client library and a reachable
database instance. They are configured through environment variables and are
**skipped** (not failed) when not set:

| Variable | Purpose |
|---|---|
| `YASCLI_HOME` | Directory containing the client library (`yascli.dll` / `libyascli.so`) |
| `YASDB_URL` | Database address, e.g. `172.16.91.21:16888` |
| `YASDB_USER` | Database username |
| `YASDB_PASSWORD` | Database password |

```sh
YASCLI_HOME=/path/to/client/lib \
YASDB_URL=127.0.0.1:1688 \
YASDB_USER=yashan \
YASDB_PASSWORD=yashan \
cargo test
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
