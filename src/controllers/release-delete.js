/** Delete a release, keeping its tag. */
import { z } from "zod";
import { github, parseRepo, refName } from "../services/api.js";
import { optionalInt } from "../models/args.js";
import { Refused } from "../utils/errors.js";
import { findReleaseByTag } from "../services/releases.js";
import { DESTROY, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "release_delete",
  description:
    "Delete a release of a GitHub repository, by tag or by id. Destructive: its notes and assets are " +
    "lost, so confirm with the owner first. The tag is kept; use tag_delete to remove that too. " +
    "Finds drafts by tag as well.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().optional().describe("Tag of the release to delete. Give this or release_id."),
    release_id: z.number().int().optional().describe("Id of the release (from release_list). Give this or tag."),
  },
  outputSchema: {
    deleted: z.boolean(),
    id: z.number(),
    tag: z.string().optional(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, tag?: unknown, release_id?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = args.tag === undefined || args.tag === null ? undefined : refName(args.tag, "tag");
  const given = optionalInt(args.release_id, "release_id", { min: 1 });
  if ((tag === undefined) === (given === undefined)) {
    throw new Refused("Give exactly one of `tag` and `release_id`.");
  }

  const id = given ?? (await findReleaseByTag(repo, /** @type {string} */ (tag))).id;
  await github("DELETE", `${repo.path}/releases/${id}`, {
    what: `delete the release ${tag ? `for '${tag}'` : id} of ${repo.full}`,
  });

  return {
    payload: {
      summary: `Deleted the release ${tag ? `for ${tag}` : id} of ${repo.full}. Its tag was kept.`,
      deleted: true,
      id,
      ...(tag === undefined ? {} : { tag }),
    },
    ok: true,
  };
}
