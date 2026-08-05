# AGENTS.md

## Project

- Single Rust crate (`yashandb`), edition 2024, MSRV 1.95; it dynamically loads the C `yascli` client with `libloading`.
- No CI, workspace, feature flags, or lint/task-runner configuration is present. `rustfmt.toml` sets `max_width = 120`.

## Commands

- Format: `cargo fmt` (check only with `cargo fmt --check`).
- Lint: `cargo clippy --all-targets`.
- Build: `cargo build`.
- Unit tests: `cargo test --lib`.
- One integration test binary: `cargo test --test library`, `cargo test --test connect`, or `cargo test --test connection_attr`.
- Full suite: `cargo test`.
- Integration tests use the real client/database only when configured; otherwise they print a skip message and return successfully. To exercise them, set `YASCLI_HOME` (directory containing `yascli.dll` on Windows or `libyascli.so` elsewhere), `YASDB_URL`, `YASDB_USER`, and `YASDB_PASSWORD` before `cargo test`.

## Architecture

- `src/ffi/raw.rs` is the only location for declarations that mirror `yacli.h`: C types, `extern "C"`/function pointers, `#[repr(C)]` structs, and enum values. Keep its `#![allow(dead_code)]` scoped there; raw enum variants drop the C prefix and types retain `Yac`.
- FFI capabilities are separate modules implementing `YacLib` (`attr`, `conn`, `diag`). Every new dynamic symbol must be resolved in `YacLib::from_loaded` in `src/ffi/mod.rs`; symbols are required at load time so missing ones fail fast.
- `src/load.rs` tries the bare platform library name first, then `%USERPROFILE%\\.yashandb\\client\\lib` on Windows or `$HOME/.yashandb/client/lib` elsewhere. Bundled dependencies are preloaded there; `Loaded` declares the main library before dependency handles so the main library drops first.
- `src/library.rs` owns the process-global `YAC_LIB` singleton. First load wins: a different explicit path returns `Error::ClientLibrary`; failed loads leave the singleton unset for retry. `Connection::connect` must keep its lazy auto-load behavior.
- `Connection` is `Send` but not `Sync`; FFI handle methods intentionally take `&mut` even for reads to enforce exclusive access.
- `YacResult::Success` and `SuccessWithInfo` are successful. Other results are converted through diagnostics into `Error::Database`.

## Constraints

- `ConnectionBuilder::connect` sets the environment charset to UTF-8 before connecting. Do not assume the client default encoding is usable for Rust strings.
- Connection parameters are passed with `i16` byte lengths; `conn_param_len` rejects oversized strings with `Error::InvalidArgument`.
- `Error` is `#[non_exhaustive]`; external matches must include a wildcard.
- `src/lib.rs` enables `#![warn(missing_docs)]`; document every new public item.

## Test Concurrency

- Library state is process-global. Tests that load/connect must serialize through their binary's `LIB_LOCK` and use the existing skip-if-unset helpers in `tests/common/mod.rs`; follow that pattern for new integration tests.
