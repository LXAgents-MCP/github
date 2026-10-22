/** Delete a branch. */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../services/api.js";
import { DESTROY, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "branch_delete",
  description:
    "Delete a branch from a GitHub repository. Destructive: commits only that branch holds become " +
    "unreachable, so confirm with the owner first. The default branch cannot be deleted.",
  inputSchema: {
    repo: repoArg,
    branch: z.string().describe("Name of the branch to delete."),
  },
  outputSchema: {
    deleted: z.boolean(),
    branch: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, branch?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const branch = refName(args.branch, "branch");

  await github("DELETE", `${repo.path}/git/refs/heads/${refPath(branch)}`, {
    what: `delete the branch '${branch}' from ${repo.full}`,
    explain: (status, detail) =>
      status === 422
        ? `GitHub would not delete the branch '${branch}' from ${repo.full} (422)${detail}. The branch ` +
          "may not exist, or it is the default branch, which cannot be deleted until another branch " +
          "is made the default."
        : undefined,
  });

  return {
    payload: { summary: `Deleted branch ${branch} from ${repo.full}.`, deleted: true, branch },
    ok: true,
  };
}
