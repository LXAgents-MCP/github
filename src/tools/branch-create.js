/** Create a branch. */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../api.js";
import { Refused } from "../errors.js";
import { WRITE, repoArg } from "../schemas.js";
import { short } from "../text.js";

const SHA = /^[0-9a-f]{40,64}$/;

export const TOOL = {
  name: "branch_create",
  description:
    "Create a branch in a GitHub repository, pointing at a branch, tag or commit. Use it to start " +
    "work off the default branch or off another branch. Fails if the name is taken.",
  inputSchema: {
    repo: repoArg,
    branch: z.string().describe("Name of the new branch, e.g. 'feat/login'."),
    from: z
      .string()
      .optional()
      .describe("Branch, tag or commit sha to start from. Default: the repository's default branch."),
  },
  outputSchema: {
    branch: z.string(),
    sha: z.string(),
    from: z.string().optional(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, branch?: unknown, from?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const branch = refName(args.branch, "branch");
  const from = args.from === undefined || args.from === null ? undefined : refName(args.from, "from");

  // The sha media type answers with the commit's sha and nothing else, where the default answer
  // is the whole commit with its file list - far more than the one fact needed here.
  const start = await github("GET", `${repo.path}/commits/${from === undefined ? "HEAD" : refPath(from)}`, {
    accept: "application/vnd.github.sha",
    raw: true,
    what: `find ${from === undefined ? "the default branch" : `'${from}'`} in ${repo.full} to branch from`,
    explain: (status) =>
      status === 409
        ? `${repo.full} is empty, so there is nothing to branch from. Create its first commit with file_create.`
        : undefined,
  });
  const sha = typeof start.body === "string" ? start.body.trim() : "";
  if (!SHA.test(sha)) {
    throw new Refused(`GitHub answered, but the body was not a commit sha this server recognizes.`);
  }

  await github("POST", `${repo.path}/git/refs`, {
    json: { ref: `refs/heads/${branch}`, sha },
    what: `create the branch '${branch}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 422 && /already exists/i.test(detail)
        ? `GitHub could not create the branch '${branch}' (422)${detail}. A branch with that name already exists in ${repo.full}; pick another name.`
        : undefined,
  });

  return {
    payload: {
      summary: `Created branch ${branch} at ${short(sha)} in ${repo.full}${from === undefined ? "" : `, from ${from}`}.`,
      branch,
      sha,
      ...(from === undefined ? {} : { from }),
    },
    ok: true,
  };
}
