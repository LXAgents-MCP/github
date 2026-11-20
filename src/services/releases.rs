//! Finding a release by its tag, which the release tools share.
//!
//! GitHub serves a release by tag only once it is published, so a draft cannot be fetched that way.
//! Drafts do appear in the release list for a token with push access, so when the direct lookup finds
//! nothing the list is searched. Without this, a draft could be created and then neither read nor deleted
//! by the same name.
use serde_json::Value;

use super::api::{GitHubOptions, github};
use super::context::Ctx;
use crate::models::address::{Repo, ref_path};
use crate::models::body::{get, list};
use crate::utils::errors::Refused;

const PAGES_SEARCHED: i64 = 3;
const PAGE_SIZE: usize = 100;

/// The release for `tag` (a name already accepted by `ref_name`), as GitHub returns it.
pub async fn find_release_by_tag(ctx: &Ctx, repo: &Repo, tag: &str) -> Result<Value, Refused> {
    let direct = github(
        ctx,
        "GET",
        &format!("{}/releases/tags/{}", repo.path, ref_path(tag)),
        GitHubOptions::what(format!(
            "read the release for the tag '{tag}' of {}",
            repo.full
        ))
        .allow(&[404]),
    )
    .await?;
    if direct.status == 200 {
        return Ok(direct.body);
    }

    for page in 1..=PAGES_SEARCHED {
        let answer = github(
            ctx,
            "GET",
            &format!("{}/releases", repo.path),
            GitHubOptions::what(format!(
                "list the releases of {} to find the one for '{tag}'",
                repo.full
            ))
            .query("per_page", PAGE_SIZE)
            .query("page", page),
        )
        .await?;
        let releases = list(&answer.body);
        if let Some(found) = releases
            .iter()
            .find(|release| get(release, "tag_name").as_str() == Some(tag))
        {
            return Ok(found.clone());
        }
        if releases.len() < PAGE_SIZE {
            break;
        }
    }
    Err(Refused(format!(
        "There is no release for the tag '{tag}' in {}. Check the tag name, or list them with release_list.",
        repo.full
    )))
}
