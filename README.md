# YashanDB Rust Driver

An official Rust driver for [YashanDB](https://www.yashandb.com). It provides a
synchronous, blocking connection to a YashanDB instance.

## Features

- **Blocking `Connection`** to a YashanDB instance, with automatic resource
  management.
- **Automatic client library loading** — the YashanDB client library is loaded
  on first use, with optional explicit-path loading.

## MSRV

The minimum supported Rust version is **1.95**.

## Requirements

- A YashanDB client library (`yascli`) installed on the machine. The driver
  finds it automatically from the default search path, or from the per-user
  install location `$HOME/.yashandb/client/lib` (Linux/macOS) or
  `%USERPROFILE%\.yashandb\client\lib` (Windows).
- A reachable YashanDB instance to connect to.

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
    let conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
    // ... use the connection ...
    Ok(())
}
```

The client library is loaded automatically on first use. To fail fast at
startup, or to load it from a specific location, call `load_library` first:

```rust
use yashandb::{load_library, load_library_with_path, Connection};

// Auto-discovery: default search path, then the per-user install directory.
load_library()?;

// Or an explicit path.
load_library_with_path("/opt/yashandb/client/lib/yascli.so")?;

let conn = Connection::connect("127.0.0.1:1688", "yashan", "yashan")?;
// ...
```

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
