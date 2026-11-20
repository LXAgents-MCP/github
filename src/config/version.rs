//! Who this server is. The version is the package's own, fixed at build time, so nothing is read from
//! disk when the server starts.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SERVER_NAME: &str = "lxagents-github";
