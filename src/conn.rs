//! Blocking connection to a YashanDB instance.

use crate::error::Error;
use crate::ffi;
use crate::library;

/// A synchronous blocking connection to a YashanDB instance.
///
/// `Send` so the connection can be moved to a worker thread; `!Sync` because the
/// underlying C handles must not be accessed concurrently from multiple threads.
pub struct Connection {
    env: ffi::EnvHandle,
    dbc: ffi::DbcHandle,
}

impl Connection {
    /// Connect to a YashanDB instance.
    ///
    /// Returns a `Connection` only on success.
    ///
    /// # URL format
    ///
    /// The `url` argument supports the following formats:
    ///
    /// - Single address: `host:port`
    /// - Multiple addresses: `serverType:host:port,host:port,host:port`
    ///   (addresses separated by `,`)
    /// - Multiple address groups:
    ///   `serverType:host:port,host:port;host:port,host:port`
    ///   (groups separated by `;`, addresses within a group separated by `,`)
    ///
    /// `host` may be an IPv4 address, IPv6 address, or domain name. `port` is
    /// the server listening port, defaulting to `1688`.
    ///
    /// `serverType` is optional and may be one of:
    /// - `primary` (default): connect addresses in order, keep the first
    ///   connection to a primary node
    /// - `standby`: connect addresses in order, keep the first connection to a
    ///   standby node
    /// - `loadBalance`: shuffle addresses, pick the node with the fewest
    ///   sessions
    /// - `primaryLoadBalance`: shuffle addresses, pick the primary node with
    ///   the fewest sessions
    /// - `standbyLoadBalance`: shuffle addresses, pick the standby node with
    ///   the fewest sessions
    pub fn connect(url: &str, username: &str, password: &str) -> Result<Self, Error> {
        let lib = library::library(None)?;

        let mut env = lib.alloc_env()?;
        let mut dbc = match lib.alloc_dbc(&mut env) {
            Ok(dbc) => dbc,
            Err(e) => {
                lib.free_env(&mut env);
                return Err(e);
            }
        };

        if let Err(e) = lib.connect(&mut dbc, url, username, password) {
            lib.free_dbc(&mut dbc);
            lib.free_env(&mut env);
            return Err(e);
        }

        Ok(Self { env, dbc })
    }
}

/// SAFETY: `Connection` owns its raw handles exclusively and is `!Sync` (the raw
/// pointers are not `Sync`), so the C driver is only ever invoked from a single
/// thread at a time. Moving it between threads (`Send`) is safe because each
/// connection uses its own distinct env/dbc handles and the C functions do not
/// require thread affinity (diagnostics use per-thread buffers read on the same
/// thread that made the call).
unsafe impl Send for Connection {}

impl Drop for Connection {
    #[inline]
    fn drop(&mut self) {
        let lib = library::loaded_library();
        lib.disconnect(&mut self.dbc);
        lib.free_dbc(&mut self.dbc);
        lib.free_env(&mut self.env);
    }
}
