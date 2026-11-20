//! The repository contents API, which the file tools share.
//!
//! GitHub changes a file by `PUT` or `DELETE` against its path and wants the blob sha of the version being
//! replaced, so a tool that is not handed one has to read it first. That read, the path, and the encoding
//! of what is written live here once.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::api::{GitHubOptions, github};
use super::context::Ctx;
use crate::config::config::MAX_WRITE_BYTES;
use crate::models::address::{FilePath, Repo};
use crate::models::body::get;
use crate::utils::errors::Refused;

/// The API path of a file or directory; an empty path is the root.
pub fn contents_path(repo: &Repo, file: &FilePath) -> String {
    if file.encoded.is_empty() {
        format!("{}/contents", repo.path)
    } else {
        format!("{}/contents/{}", repo.path, file.encoded)
    }
}

/// Content as the API wants it written, base64, refusing what is too large to be a source file.
pub fn encode_content(content: &str) -> Result<String, Refused> {
    let bytes = content.as_bytes();
    if bytes.len() > MAX_WRITE_BYTES {
        return Err(Refused(format!(
            "`content` is {} bytes, over the {MAX_WRITE_BYTES}-byte limit for one write. This tool writes source files; commit large files with git.",
            bytes.len()
        )));
    }
    Ok(STANDARD.encode(bytes))
}

/// The blob sha of the file at `path`, read so a change can say which version it replaces.
///
/// `reference` is the branch to read it on; `None` is the default branch. `instead` is what to suggest
/// when there is no such file.
pub async fn blob_sha(
    ctx: &Ctx,
    repo: &Repo,
    file: &FilePath,
    reference: Option<&str>,
    instead: &str,
) -> Result<String, Refused> {
    let explain = |status: u16, _: &str| {
        (status == 404).then(|| {
            let on = reference.map(|r| format!(" on {r}")).unwrap_or_default();
            format!(
                "There is no file '{}' in {}{on}. {instead}",
                file.path, repo.full
            )
        })
    };
    let answer = github(
        ctx,
        "GET",
        &contents_path(repo, file),
        GitHubOptions::what(format!(
            "read '{}' in {} to find the version it replaces",
            file.path, repo.full
        ))
        .query("ref", reference)
        .explain(&explain),
    )
    .await?;
    let body = &answer.body;
    if body.is_array() {
        return Err(Refused(format!(
            "`path` '{}' is a directory in {}, not a file.",
            file.path, repo.full
        )));
    }
    match (get(body, "type").as_str(), get(body, "sha").as_str()) {
        (Some("file"), Some(sha)) => Ok(sha.to_string()),
        _ => Err(Refused(format!(
            "`path` '{}' in {} is not a regular file, so it cannot be replaced.",
            file.path, repo.full
        ))),
    }
}
