//! Reading the arguments that end up in a GitHub URL: a repository, a reference, a path in a repository,
//! a workflow. Each is validated here and returned with the form it takes in a URL, so
//! `src/services/api.rs` can build a path from nothing that has not passed through this file.
use std::sync::LazyLock;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use serde_json::Value;

use super::args::{raw_text, required_text};
use crate::utils::errors::Refused;

// GitHub's own rules: a login is letters, digits and '-' (managed accounts add '_'), up to 39
// characters; a repository name is letters, digits, '-', '_' and '.', up to 100.
static OWNER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,38}$").unwrap());

pub static REPO_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._-]{1,100}$").unwrap());

/// What `encodeURIComponent` leaves alone: letters, digits and `-_.!~*'()`.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// `encodeURIComponent`.
pub fn encode_component(text: &str) -> String {
    utf8_percent_encode(text, COMPONENT).to_string()
}

/// A repository argument, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
    /// `owner/name`.
    pub full: String,
    /// The API prefix, `/repos/owner/name`, built only from characters that need no encoding.
    pub path: String,
}

/// Read a repository argument, `owner/name`.
pub fn parse_repo(value: &Value, field: &str) -> Result<Repo, Refused> {
    let text = required_text(value, field)?;
    let parts: Vec<&str> = text.split('/').collect();
    let valid = parts.len() == 2
        && OWNER.is_match(parts[0])
        && REPO_NAME.is_match(parts[1])
        && parts[1] != "."
        && parts[1] != "..";
    if !valid {
        return Err(Refused(format!(
            "`{field}` '{text}' is not a repository. Write it as 'owner/name', for example 'LXAgents-MCP/github'."
        )));
    }
    let (owner, name) = (parts[0].to_string(), parts[1].to_string());
    Ok(Repo {
        full: format!("{owner}/{name}"),
        path: format!("/repos/{owner}/{name}"),
        owner,
        name,
    })
}

/// Characters git forbids in a reference name, plus whitespace and control characters.
fn bad_ref_char(c: char) -> bool {
    c <= ' ' || c == '\u{7f}' || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
}

/// Read a branch, tag or commit argument.
///
/// Strict enough that the value cannot change what path it lands in - no `..`, no empty or dotted
/// segment, nothing git itself refuses - and loose enough to take any name git allows.
pub fn ref_name(value: &Value, field: &str) -> Result<String, Refused> {
    let name = required_text(value, field)?;
    let invalid = name.encode_utf16().count() > 255
        || name.chars().any(bad_ref_char)
        || name.contains("..")
        || name.contains("@{")
        || name.starts_with('-')
        || name.ends_with('.')
        || name.split('/').any(|segment| {
            segment.is_empty() || segment.starts_with('.') || segment.ends_with(".lock")
        });
    if invalid {
        return Err(Refused(format!(
            "`{field}` '{name}' is not a valid branch, tag or commit name."
        )));
    }
    Ok(name)
}

/// An optional reference: `None` when absent, else `ref_name`.
pub fn optional_ref(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    if value.is_null() {
        Ok(None)
    } else {
        ref_name(value, field).map(Some)
    }
}

/// A reference as it goes into a URL path: every segment encoded, the slashes kept, because GitHub reads
/// `feature/login` as one name spread over two segments. `name` is one already accepted by `ref_name`.
pub fn ref_path(name: &str) -> String {
    name.split('/')
        .map(encode_component)
        .collect::<Vec<_>>()
        .join("/")
}

/// A path inside a repository, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePath {
    /// The cleaned path.
    pub path: String,
    /// The same as it goes into a URL.
    pub encoded: String,
}

/// Read a path inside a repository. `allow_empty` makes the repository root a valid answer.
pub fn file_path(value: &Value, field: &str, allow_empty: bool) -> Result<FilePath, Refused> {
    let raw = raw_text(value, field)?.unwrap_or_default();
    let trimmed = raw
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if trimmed.is_empty() {
        if allow_empty {
            return Ok(FilePath {
                path: String::new(),
                encoded: String::new(),
            });
        }
        return Err(Refused(format!(
            "`{field}` is required: a path inside the repository, such as 'src/index.js'."
        )));
    }
    let segments: Vec<&str> = trimmed.split('/').collect();
    let control = trimmed.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}');
    if control
        || segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return Err(Refused(format!(
            "`{field}` '{trimmed}' is not a path inside the repository: no empty, '.' or '..' segments."
        )));
    }
    let encoded = segments
        .iter()
        .map(|s| encode_component(s))
        .collect::<Vec<_>>()
        .join("/");
    Ok(FilePath {
        path: trimmed,
        encoded,
    })
}

static WORKFLOW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$").unwrap());

/// A workflow argument, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workflow {
    pub name: String,
    pub encoded: String,
}

/// Read a workflow argument: its numeric id, or the name of its file under `.github/workflows/`.
///
/// A path is accepted and reduced to the file name, because that is how a workflow is written everywhere
/// else. Nothing else can reach a URL: no slash, and a name starts with a letter or a digit.
pub fn workflow_name(value: &Value, field: &str) -> Result<Workflow, Refused> {
    let text = required_text(value, field)?;
    let name = text
        .strip_prefix(".github/workflows/")
        .unwrap_or(&text)
        .to_string();
    if !WORKFLOW.is_match(&name) {
        return Err(Refused(format!(
            "`{field}` '{name}' is not a workflow. Give its numeric id, or the name of its file such as 'ci.yml' (see workflow_list)."
        )));
    }
    Ok(Workflow {
        encoded: encode_component(&name),
        name,
    })
}
