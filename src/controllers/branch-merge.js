/** Merge one branch into another, without a pull request. */
import { z } from "zod";
import { github, parseRepo, refName } from "../services/api.js";
import { optionalText } from "../models/args.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import { firstLine, short } from "../utils/text.js";

export const TOOL = {
  name: "branch_merge",
  description:
    "Merge a branch (or commit) into another branch of a GitHub repository directly, with no pull " +
    "request. Destructive: it moves the base branch, so confirm with the owner first. Use pr_merge " +
    "when a pull request exists.",
  inputSchema: {
    repo: repoArg,
    base: z.string().describe("Branch that receives the merge, e.g. 'main'."),
    head: z.string().describe("Branch or commit sha to merge into the base."),
    message: z.string().optional().describe("Commit message for the merge commit. Default: GitHub's."),
  },
  outputSchema: {
    merged: z.boolean(),
    up_to_date: z.boolean(),
    sha: z.string().optional(),
    message: z.string().optional(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, base?: unknown, head?: unknown, message?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const base = refName(args.base, "base");
  const head = refName(args.head, "head");
  const message = optionalText(args.message, "message");

  const { status, body } = await github("POST", `${repo.path}/merges`, {
    json: { base, head, ...(message === undefined ? {} : { commit_message: message }) },
    what: `merge '${head}' into '${base}' in ${repo.full}`,
    explain: (code, detail) => {
      if (code === 409) {
        return (
          `GitHub could not merge '${head}' into '${base}' in ${repo.full} (409)${detail}. The branches ` +
          `conflict. Open a pull request to resolve it there, or merge '${base}' into '${head}' first.`
        );
      }
      if (code === 404) {
        return (
          `GitHub could not find '${base}' or '${head}' in ${repo.full} (404)${detail}. Check both ` +
          "names, and that the token can see the repository."
        );
      }
      return undefined;
    },
  });

  // 204 is GitHub saying the base already contains the head: nothing was merged, and that is fine.
  if (status === 204) {
    return {
      payload: { summary: `Nothing to merge: ${base} already contains ${head}.`, merged: false, up_to_date: true },
      ok: true,
    };
  }
  const sha = typeof body?.sha === "string" ? body.sha : undefined;
  const text = firstLine(body?.commit?.message);
  return {
    payload: {
      summary: `Merged ${head} into ${base} in ${repo.full}${sha ? `, at ${short(sha)}` : ""}.`,
      merged: true,
      up_to_date: false,
      ...(sha ? { sha } : {}),
      ...(text ? { message: text } : {}),
    },
    ok: true,
  };
}
