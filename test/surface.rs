//! The tool surface this server is meant to serve, pinned in one place.
//!
//! Adding, renaming or removing a tool is a change to a public surface and a release, so the list is
//! written out and not derived: the registry, protocol and stdio tests all compare against it.
//!
//!   read     only reads
//!   write    creates something; repeating the call repeats the effect
//!   update   changes something that exists; repeating it changes nothing more
//!   destroy  deletes, overwrites or merges
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Read,
    Write,
    Update,
    Destroy,
}

use Kind::*;

pub const SURFACE: &[(&str, Kind)] = &[
    ("repo_create", Write),
    ("repo_read", Read),
    ("repo_update", Update),
    ("repo_delete", Destroy),
    ("repo_fork", Write),
    ("repo_search", Read),
    ("branch_create", Write),
    ("branch_read", Read),
    ("branch_list", Read),
    ("branch_delete", Destroy),
    ("branch_merge", Destroy),
    ("file_read", Read),
    ("file_create", Write),
    ("file_update", Update),
    ("file_delete", Destroy),
    ("directory_list", Read),
    ("commit_read", Read),
    ("commit_list", Read),
    ("commit_compare", Read),
    ("tag_check", Read),
    ("tag_get", Read),
    ("tag_get_all", Read),
    ("tag_delete", Destroy),
    ("release_create", Write),
    ("release_read", Read),
    ("release_list", Read),
    ("release_delete", Destroy),
    ("issue_open", Write),
    ("issue_read", Read),
    ("issue_comment", Write),
    ("issue_close", Update),
    ("issue_assign", Update),
    ("issue_label", Update),
    ("issue_lock", Update),
    ("pr_open", Write),
    ("pr_read", Read),
    ("pr_comment", Write),
    ("pr_close", Update),
    ("pr_merge", Destroy),
    ("pr_review_request", Update),
    ("pr_approve", Write),
    ("workflow_list", Read),
    ("workflow_run", Write),
    ("workflow_status", Read),
    ("workflow_logs", Read),
];

/// The hints each kind must carry, as (readOnly, destructive, idempotent, openWorld). `openWorld`
/// is true for all: every tool talks to GitHub.
pub fn kind_hints(kind: Kind) -> (bool, bool, bool, bool) {
    match kind {
        Read => (true, false, true, true),
        Write => (false, false, false, true),
        Update => (false, false, true, true),
        Destroy => (false, true, true, true),
    }
}

/// Tools that do not take a `repo`: one creates a repository, one searches across all of them.
pub const NO_REPO_ARGUMENT: &[&str] = &["repo_create", "repo_search"];

/// The surface's names, sorted.
pub fn tool_names() -> Vec<&'static str> {
    let mut names: Vec<_> = SURFACE.iter().map(|(name, _)| *name).collect();
    names.sort();
    names
}

/// A tool's kind.
pub fn kind_of(name: &str) -> Option<Kind> {
    SURFACE.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}
