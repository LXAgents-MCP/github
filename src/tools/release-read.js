/** Read one release. */
import { z } from "zod";
import { github, parseRepo, refName } from "../api.js";
import { READ, repoArg } from "../schemas.js";
import { findReleaseByTag } from "../releases.js";
import { describeRelease, expectBody, releaseFields, releaseShape } from "../shapes.js";

export const TOOL = {
  name: "release_read",
  description:
    "Read one release of a GitHub repository: title, notes, whether it is a draft or prerelease, and " +
    "its assets. Give a tag, or omit it for the latest published release. Finds drafts by tag too.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().optional().describe("Tag of the release. Omit for the latest published, non-prerelease one."),
  },
  outputSchema: releaseShape,
  annotations: READ,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = args.tag === undefined || args.tag === null ? undefined : refName(args.tag, "tag");

  const release =
    tag === undefined
      ? (
          await github("GET", `${repo.path}/releases/latest`, {
            what: `read the latest release of ${repo.full}`,
            explain: (status) =>
              status === 404 ? `${repo.full} has no published release yet. List what exists with release_list.` : undefined,
          })
        ).body
      : await findReleaseByTag(repo, tag);

  expectBody(release, "html_url", "a release");
  const fields = releaseFields(release);
  return { payload: { summary: `${repo.full} has ${describeRelease(fields)}.`, ...fields }, ok: true };
}
