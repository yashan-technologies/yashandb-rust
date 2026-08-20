# AGENTS.md

## Project and Commands

- This is one blocking Rust crate (`yashandb`), edition 2024, MSRV 1.95. It dynamically loads the C `yascli` client through `libloading`; no workspace, CI, feature flags, task runner, or lint configuration exists. `rustfmt.toml` sets `max_width = 120`.
- Format with `cargo fmt` (`cargo fmt --check` to check), lint with `cargo clippy --all-targets`, build with `cargo build`, and run unit tests with `cargo test --lib`.
- Run one integration binary with `cargo test --test {library|connect|connection_attr|query|prepared|transaction}`; `cargo test` runs all tests.
- Integration tests skip successfully unless `YASCLI_HOME` (directory containing `yascli.dll` on Windows or `libyascli.so` elsewhere), `YASDB_URL`, `YASDB_USER`, and `YASDB_PASSWORD` are set. Tests that load or connect must take that binary's `LIB_LOCK` and use `tests/common/mod.rs` skip helpers because library loading is process-global.

## FFI and Resource Boundaries

- Put all declarations mirroring `yacli.h` only in `src/ffi/raw.rs`: C types, function pointers, `#[repr(C)]` structs, and enum values. Keep the scoped `#![allow(dead_code)]` there; raw enum variants omit the C prefix and types retain `Yac`.
- FFI capability modules implement `YacLib` (`attr`, `conn`, `diag`, `stmt`). Resolve every new dynamic symbol in `YacLib::from_loaded` in `src/ffi/mod.rs`; symbols are mandatory and must fail at load time when unavailable.
- Keep `ffi` a thin ABI layer. Safe-layer modules must not expose or manipulate native handles such as `StmtHandle`; `Statement<'conn>` owns the handle and releases it through RAII. `Statement::finish(self)` must report release errors without allowing `Drop` to free it again.
- FFI handle methods intentionally take `&mut`, including reads, to enforce exclusive native-handle access. `Connection` is `Send` but not `Sync`.
- `YacResult::Success` and `SuccessWithInfo` are successful; map all other results through diagnostics to `Error::Database`.
- `src/library.rs` owns the global client singleton: the first successful load wins, a different explicit path returns `Error::ClientLibrary`, and failures leave it retryable. Preserve `Connection::connect` lazy loading. `src/load.rs` tries the bare platform name, then the per-user client directory; keep the main library declared before dependency handles so it drops first.

## Safe API Invariants

- `Connection::{execute,query}` only create a `Statement` and delegate. Keep C-shaped names such as `direct_execute` in `ffi`; the safe API uses operation names. A `ResultSet` or prepared-query result must be finished or dropped before reusing its statement.
- Binding belongs in `src/param/`: `BindParam` owns/borrows client buffers and decodes output after execution. Do not let client buffer pointers escape the call. Positional and named bind counts must match the client-reported count; named parameters are unique NUL-terminated `CStr` names without `:`.
- `String` and `Vec<u8>` output capacity is the client output limit. Nullable variable-length output requires `(&mut Option<String>, capacity)` or `(&mut Option<Vec<u8>>, capacity)`. Output and input/output targets may be partially updated after an error.
- New connections have manual commit enabled (`auto_commit == false`). `Connection::commit` and `rollback` do not change that setting. `Connection::transaction` is a non-nestable guard over the current connection transaction, not a server `BEGIN` or independent transaction.
- A transaction created while auto-commit is enabled disables it and restores it after `commit`, `rollback`, or drop. In manual-commit mode, it includes work pending before guard creation. An unfinished guard attempts rollback but drops its error; use explicit `rollback` when cleanup errors matter. Statements and result sets borrowed from the guard must finish or drop before completion, and direct transaction-control SQL is the caller's responsibility.
- Public database types and column metadata live in `src/types.rs`; `ColumnInfo` contains only name, `DataTypeInfo`, and nullability. `ConnectionBuilder::connect` must set UTF-8 before connecting. Connection argument byte lengths use `i16` and oversized strings return `Error::InvalidArgument`.

## Style Constraints

- `src/lib.rs` warns on missing public docs. Add documentation for every public item; do not suppress warnings outside the raw ABI mapping. `Error` is `#[non_exhaustive]`, so external matches require a wildcard arm.
- Use `#[inline]` for small forwarding methods/accessors and `const fn` plus `#[inline]` for const-safe public accessors. Append crate-internal helpers to the end of an existing `impl` unless local placement requires otherwise.
