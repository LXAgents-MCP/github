/**
 * Finding a release by its tag, which the release tools share.
 *
 * GitHub serves a release by tag only once it is published, so a draft cannot be fetched that way.
 * Drafts do appear in the release list for a token with push access, so when the direct lookup
 * finds nothing the list is searched. Without this, a draft could be created and then neither read
 * nor deleted by the same name.
 */
import { github, refPath } from "./api.js";
import { Refused } from "./errors.js";

const PAGES_SEARCHED = 3;
const PAGE_SIZE = 100;

/**
 * @param {{ full: string, path: string }} repo
 * @param {string} tag A tag name already accepted by `refName`.
 * @returns {Promise<any>} The release as GitHub returns it.
 */
export async function findReleaseByTag(repo, tag) {
  const direct = await github("GET", `${repo.path}/releases/tags/${refPath(tag)}`, {
    allow: [404],
    what: `read the release for the tag '${tag}' of ${repo.full}`,
  });
  if (direct.status === 200) return direct.body;

  for (let page = 1; page <= PAGES_SEARCHED; page += 1) {
    const { body } = await github("GET", `${repo.path}/releases`, {
      query: { per_page: PAGE_SIZE, page },
      what: `list the releases of ${repo.full} to find the one for '${tag}'`,
    });
    const releases = Array.isArray(body) ? body : [];
    const found = releases.find((release) => release.tag_name === tag);
    if (found) return found;
    if (releases.length < PAGE_SIZE) break;
  }
  throw new Refused(
    `There is no release for the tag '${tag}' in ${repo.full}. Check the tag name, or list them with release_list.`,
  );
}
