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

### Executing SQL and reading rows

`execute` is for non-parameterized SQL that does not return rows. `query`
returns a streaming `ResultSet`; it exclusively borrows the connection until
the result set is dropped or `finish`ed.

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
name. Supported SQL values include booleans, signed integer and floating-point
types, `NUMBER`, date/time and interval types, text, and binary data. Wrap a
target in `Option<T>` to read a database `NULL`. `TIMESTAMP WITH [LOCAL] TIME
ZONE` and unrecognized types remain visible in metadata but return an error only
if that column is read.

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
