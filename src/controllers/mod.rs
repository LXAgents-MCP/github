//! Tool registry: one module per tool, each registered here.
//!
//! Adding a tool is a new file in this directory that exports `def()` (name, description, schemas and
//! hints) and `handle`, plus one line below. The file name is the tool name: `repo_create.rs` is
//! `repo_create`, and a test holds the two together in both directions. `src/routes/server.rs` never
//! names a tool, so the protocol layer cannot grow a per-tool special case. A module that is not
//! registered is never callable.
use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::models::tool::ToolSpec;

pub mod branch_create;
pub mod branch_delete;
pub mod branch_list;
pub mod branch_merge;
pub mod branch_read;
pub mod commit_compare;
pub mod commit_list;
pub mod commit_read;
pub mod directory_list;
pub mod file_create;
pub mod file_delete;
pub mod file_read;
pub mod file_update;
pub mod issue_assign;
pub mod issue_close;
pub mod issue_comment;
pub mod issue_label;
pub mod issue_lock;
pub mod issue_open;
pub mod issue_read;
pub mod pr_approve;
pub mod pr_close;
pub mod pr_comment;
pub mod pr_merge;
pub mod pr_open;
pub mod pr_read;
pub mod pr_review_request;
pub mod release_create;
pub mod release_delete;
pub mod release_list;
pub mod release_read;
pub mod repo_create;
pub mod repo_delete;
pub mod repo_fork;
pub mod repo_read;
pub mod repo_search;
pub mod repo_update;
pub mod tag_check;
pub mod tag_delete;
pub mod tag_get;
pub mod tag_get_all;
pub mod workflow_list;
pub mod workflow_logs;
pub mod workflow_run;
pub mod workflow_status;

macro_rules! spec {
    ($module:ident) => {
        ToolSpec {
            name: stringify!($module),
            def: $module::def(),
            handle: $module::handle,
        }
    };
}

/// Every registered tool, in name order.
fn registered() -> Vec<ToolSpec> {
    vec![
        spec!(branch_create),
        spec!(branch_delete),
        spec!(branch_list),
        spec!(branch_merge),
        spec!(branch_read),
        spec!(commit_compare),
        spec!(commit_list),
        spec!(commit_read),
        spec!(directory_list),
        spec!(file_create),
        spec!(file_delete),
        spec!(file_read),
        spec!(file_update),
        spec!(issue_assign),
        spec!(issue_close),
        spec!(issue_comment),
        spec!(issue_label),
        spec!(issue_lock),
        spec!(issue_open),
        spec!(issue_read),
        spec!(pr_approve),
        spec!(pr_close),
        spec!(pr_comment),
        spec!(pr_merge),
        spec!(pr_open),
        spec!(pr_read),
        spec!(pr_review_request),
        spec!(release_create),
        spec!(release_delete),
        spec!(release_list),
        spec!(release_read),
        spec!(repo_create),
        spec!(repo_delete),
        spec!(repo_fork),
        spec!(repo_read),
        spec!(repo_search),
        spec!(repo_update),
        spec!(tag_check),
        spec!(tag_delete),
        spec!(tag_get),
        spec!(tag_get_all),
        spec!(workflow_list),
        spec!(workflow_logs),
        spec!(workflow_run),
        spec!(workflow_status),
    ]
}

/// Check a set of tools for a name given twice, or a module whose tool is named differently.
///
/// Two tools answering to one name would make `tools/call` dispatch arbitrary, so the registry refuses
/// it when it is built - at start - rather than at call time.
pub fn build_registry(specs: Vec<ToolSpec>) -> Result<Vec<ToolSpec>, String> {
    let mut seen = BTreeSet::new();
    for spec in &specs {
        if spec.def.name != spec.name {
            return Err(format!(
                "{}.rs defines the tool '{}'.",
                spec.name, spec.def.name
            ));
        }
        if !seen.insert(spec.name) {
            return Err(format!("Duplicate tool name '{}'.", spec.name));
        }
    }
    Ok(specs)
}

/// The tools this server serves.
pub fn tools() -> &'static [ToolSpec] {
    static TOOLS: OnceLock<Vec<ToolSpec>> = OnceLock::new();
    TOOLS.get_or_init(|| build_registry(registered()).unwrap_or_else(|problem| panic!("{problem}")))
}

/// One tool by name.
pub fn get(name: &str) -> Option<&'static ToolSpec> {
    tools().iter().find(|spec| spec.name == name)
}
