//! Annotations shared by the tools, and the reading of a published schema.
//!
//! Each tool writes its schemas as JSON in its own module, exactly as 0.7.0 published them; the golden
//! `test/fixtures/tools-list.json` holds them to that.
use serde_json::Value;

use super::tool::Hints;

/*
 * Annotations tell the client what a tool will do, so it can decide whether to ask first. They are
 * honest or they are worse than none: a read that claims to be read-only and writes is trusted exactly
 * because it said so. `openWorldHint` is true everywhere - every tool talks to GitHub.
 */

/// Reads and changes nothing.
pub const READ: Hints = Hints {
    read_only: true,
    destructive: false,
    idempotent: true,
    open_world: true,
};

/// Creates something new; repeating the call creates it again or fails.
pub const WRITE: Hints = Hints {
    read_only: false,
    destructive: false,
    idempotent: false,
    open_world: true,
};

/// Changes something that exists; repeating the call with the same arguments changes nothing more.
pub const UPDATE: Hints = Hints {
    read_only: false,
    destructive: false,
    idempotent: true,
    open_world: true,
};

/// Deletes, overwrites or merges, which cannot be taken back from here.
pub const DESTROY: Hints = Hints {
    read_only: false,
    destructive: true,
    idempotent: true,
    open_world: true,
};

/// A schema written as JSON in a tool module. The text is the module's own constant, so a parse failure
/// is a bug in this repository and fails the registry test before it can ship.
pub fn schema(json: &str) -> Value {
    serde_json::from_str(json).expect("a tool's schema is valid JSON")
}
