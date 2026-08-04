# AGENTS.md

Rust driver for YashanDB. Wraps the C `yascli` client library via `libloading`
(dynamic loading at runtime — no linking to C headers). Single crate, no
workspace, no CI, no lint config.

## Commands

- Build/test: `cargo build`, `cargo test` (no clippy config exists; don't
  introduce one unless asked)
- Format: `cargo fmt` (rustfmt.toml sets `max_width = 120`)
- MSRV is **1.95** (edition 2024). Don't use APIs newer than 1.95; e.g.
  `OnceLock::get_or_try_init` is still not stable — `src/library.rs` hand-rolls
  the lock instead.

## Architecture

- `src/ffi/raw.rs` is the **only** place for C type/function-pointer
  declarations, `extern "C"` fns, `#[repr(C)]` structs, and C enum values. It
  maps `yacli.h`.
  - Naming: enum variants drop the C prefix (`YAC_SUCCESS` → `Success`), types
    keep the `Yac` prefix (`YacHandle`).
  - `#![allow(dead_code)]` is scoped to this module only; don't move it up.
- Each ffi capability lives in its own module (`src/ffi/connect.rs`,
  `src/ffi/diag.rs`) as `impl YacLib` methods. New symbols must be added to
  `YacLib::from_loaded` in `src/ffi/mod.rs`; all symbols resolve at load time
  so a bad library fails fast.
- `YacLib` holds four attr symbols (`set/get_env_attr`, `set/get_conn_attr`)
  that are loaded but marked `#[allow(dead_code)]` — reserved for the future
  charset/attribute API. Don't remove them as "dead code"; they exist so a
  library lacking them fails fast at load time.
- `src/library.rs` `YAC_LIB` is a process-global `OnceLock` singleton.
  **First load wins**: `load_library_with_path` with a different file after
  load returns `Error::ClientLibrary` ("refusing to load from"). A failed load
  leaves the library unloaded and retries later.
- `src/load.rs` mirrors the Go driver's discovery: bare name first, then
  `$HOME/.yashandb/client/lib` (or `%USERPROFILE%\.yashandb\client\lib`) with
  bundled dependency libs preloaded. `Loaded` field order matters (main lib
  drops before deps).
- `Connection` is `Send + !Sync`. FFI handle methods take `&mut` even when
  read-only — that exclusive borrow is what makes `Send` sound. Keep it.
- `YacResult::Success | SuccessWithInfo` = Ok; anything else goes through
  `get_diag_rec()` into `Error::Database`.
- `src/lib.rs` sets `#![warn(missing_docs)]`; new public items need doc comments.

## Constraints / gotchas

- C driver encodes strings as **GBK by default**; UTF-8 non-ASCII input/output
  is garbled. Marked `TODO(attr)` in `src/ffi/connect.rs` and `diag.rs`. Don't
  "fix" silently.
- Connection params are length-prefixed with `i16` bytes; `conn_param_len`
  rejects over-length strings with `Error::InvalidArgument`.
- `Error` is `#[non_exhaustive]`; tests match on concrete variants.
- Never add behavior that turns the lazy `connect()` auto-load into a
  hard requirement; public API is `load_library` / `load_library_with_path` /
  `Connection::connect`.

## Testing

- Most unit tests run without a library, but some in `src/load.rs` and
  `src/library.rs` also exercise the real yascli lib. Integration tests
  (`tests/`) need a real lib + a real DB and are **skipped, not failed**, when
  env is unset: `YASCLI_HOME` (dir with the lib), `YASDB_URL`, `YASDB_USER`,
  `YASDB_PASSWORD`.
- Library state is process-global, so any test touching loading/connecting
  must take the shared `LIB_LOCK` mutex and use the skip-if-unset pattern
  (see `tests/library.rs`, `tests/connect.rs`, `tests/common/mod.rs`, and the
  `#[cfg(test)]` modules in `src/load.rs` / `src/library.rs`).
- Real-library tests carry `#[cfg_attr(all(miri, ...), ignore = ...)]`; keep
  these when adding tests that load or connect.
- Run full suite with env vars:
  `YASCLI_HOME=... YASDB_URL=... YASDB_USER=... YASDB_PASSWORD=... cargo test`