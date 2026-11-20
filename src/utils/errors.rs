//! The one kind of failure a tool reports on purpose.
use std::fmt;

/// A request the owner would not want served, or one that cannot succeed. Carries a message worth
/// reading: it is what the model sees, so it says what to do next.
///
/// Anything else that goes wrong in a handler is a bug, and a bug is a panic, which the dispatcher in
/// `src/routes/server.rs` catches and reports as an internal error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl Refused {
    pub fn new(message: impl Into<String>) -> Self {
        Refused(message.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

/// Refuse with a formatted message: `refuse!("`{field}` is required.")`.
#[macro_export]
macro_rules! refuse {
    ($($arg:tt)*) => {
        return Err($crate::utils::errors::Refused(format!($($arg)*)))
    };
}
