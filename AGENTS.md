# AGENTS.md

## Project and Verification

- `yashandb` is one synchronous, blocking Rust crate (edition 2024; MSRV 1.95), not a workspace. It dynamically loads the native `yascli` client through `libloading`; there are no features, CI workflows, task runner, or lint configuration. `rustfmt.toml` sets `max_width = 120`.
- Run `cargo fmt --check`, `cargo clippy --all-targets`, `cargo build`, and `cargo test --lib` for local verification. Use `cargo test --test {library|connect|connection_attr|query|prepared|transaction|lob|json}` for one integration binary; `cargo test` runs all tests.
- Integration tests skip cleanly unless `YASCLI_HOME` names a directory containing `yascli.dll` (Windows) or `libyascli.so` (elsewhere), and `YASDB_URL`, `YASDB_USER`, and `YASDB_PASSWORD` are set. Reuse the skip/setup helpers in `tests/common/mod.rs` and unique database object names from `test_object_name`.

## Layering and FFI

- Keep C-header mappings exclusively in `src/ffi/raw.rs`: C aliases, function pointers, `#[repr(C)]` structs, and enum values. The scoped `#![allow(dead_code)]` belongs only there; raw variants omit the C prefix and types retain `Yac`.
- `src/ffi/{attr,conn,diag,lob,stmt}.rs` are thin ABI capability modules. Every native function added to one must be a mandatory symbol resolved in `YacLib::from_loaded` (`src/ffi/mod.rs`), so unsupported clients fail when loading.
- Safe-layer code must not expose or manipulate native handles. `Statement<'conn>` owns its native statement and releases it through RAII; `finish(self)` must surface release errors without allowing `Drop` to release it again. FFI handle methods deliberately take `&mut`, including reads, to enforce exclusive native-handle access.
- Treat both `YacResult::Success` and `SuccessWithInfo` as success; turn all other native results into diagnostics-backed `Error::Database` values.
- `src/library.rs` owns the process-global client: first successful load wins, a different explicit path is rejected, and failures remain retryable. Preserve lazy loading by `Connection::connect`. In `src/load.rs`, try the bare platform library name before the per-user client directory and keep the main library field before dependency handles so it drops first.

## API and Lifetime Invariants

- `Connection` is `Send` but `!Sync`. New connections start with manual commit; `commit` and `rollback` do not change auto-commit. `Transaction` is non-nestable and guards the connection's current transaction, rather than issuing `BEGIN` or creating an independent transaction.
- A transaction begun while auto-commit is enabled temporarily disables and restores it. An unfinished guard attempts rollback but discards cleanup errors, so use explicit `rollback` when that error matters. Drop or finish all statements, result sets, and transaction-borrowed LOBs before completing the guard.
- Keep `Connection::{execute,query}` as statement-creation delegates; reserve C-shaped names such as `direct_execute` for `ffi`. A `ResultSet` or prepared-query result must be finished or dropped before its statement is reused.
- Parameter storage and output decoding belong in `src/param/`; do not let client buffer pointers escape native calls. Positional and named bind counts must match the client-reported count; named parameters are unique NUL-terminated `CStr` names without `:`.
- `String` and `Vec<u8>` capacity is the client output limit. Nullable variable-length output uses `(&mut Option<String>, capacity)` or `(&mut Option<Vec<u8>>, capacity)`, and output/in-out targets can be partially updated on error. LOB and JSON `in_out` bindings are unsupported; use separate input and output bindings.
- `Blob` and `Clob` are connection-bound native locators, not materialized values. A query LOB column can be extracted only once per row; transferred locators may outlive a finished result set but must finish or drop before their connection. BLOB offsets are one-based bytes; CLOB/NCLOB offsets and lengths are one-based characters.
- Public database types are defined in `src/types.rs` and column metadata in `src/column.rs`. `ConnectionBuilder::connect` must set UTF-8 before connecting; connection argument lengths are `i16`, and oversized strings return `Error::InvalidArgument`.

## Public API Style

- `src/lib.rs` enables `#![warn(missing_docs)]`: document every public item and do not suppress that warning outside the raw ABI module. `Error` is `#[non_exhaustive]`, so external matches require a wildcard arm.
- Use `#[inline]` for small forwarding methods/accessors; use `const fn` plus `#[inline]` when a public accessor is const-safe.
