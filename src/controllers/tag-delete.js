/** Delete a tag. */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../services/api.js";
import { DESTROY, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "tag_delete",
  description:
    "Delete a tag from a GitHub repository. Destructive: anything that pins the tag, such as a " +
    "release, loses its anchor, so confirm with the owner first. Deleting a release does not delete " +
    "its tag, and this does not delete the release.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Name of the tag to delete."),
  },
  outputSchema: {
    deleted: z.boolean(),
    tag: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");

  await github("DELETE", `${repo.path}/git/refs/tags/${refPath(tag)}`, {
    what: `delete the tag '${tag}' from ${repo.full}`,
    explain: (status, detail) =>
      status === 422
        ? `GitHub would not delete the tag '${tag}' from ${repo.full} (422)${detail}. It probably does not exist; list the tags with tag_get_all.`
        : undefined,
  });

  return { payload: { summary: `Deleted tag ${tag} from ${repo.full}.`, deleted: true, tag }, ok: true };
}
