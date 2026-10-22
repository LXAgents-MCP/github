/** Merge a pull request. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { oneOf, optionalText, rawText, requiredInt } from "../models/args.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import { short } from "../utils/text.js";

const METHODS = ["merge", "squash", "rebase"];

export const TOOL = {
  name: "pr_merge",
  description:
    "Merge a pull request in a GitHub repository, by merge commit, squash or rebase. Destructive: it " +
    "moves the base branch, so confirm with the owner first. Pass `sha` to merge only if the branch is " +
    "still at the commit that was reviewed.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Pull request number."),
    method: z.enum(METHODS).optional().describe("merge (default), squash or rebase. The repository may disable some."),
    title: z.string().optional().describe("Title for the merge or squash commit. Default: GitHub's."),
    message: z.string().optional().describe("Body for the merge or squash commit. Default: GitHub's."),
    sha: z.string().optional().describe("The head commit the merge must match; a moved branch is refused (409)."),
  },
  outputSchema: {
    merged: z.boolean(),
    sha: z.string().optional(),
    message: z.string().optional(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, number?: unknown, method?: unknown, title?: unknown, message?: unknown, sha?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const method = oneOf(args.method, "method", METHODS, "merge");
  const title = optionalText(args.title, "title");
  const message = rawText(args.message, "message");
  const sha = optionalText(args.sha, "sha");

  const { body } = await github("PUT", `${repo.path}/pulls/${number}/merge`, {
    json: {
      merge_method: method,
      ...(title === undefined ? {} : { commit_title: title }),
      ...(message === undefined ? {} : { commit_message: message }),
      ...(sha === undefined ? {} : { sha }),
    },
    what: `merge pull request #${number} of ${repo.full}`,
    explain: (status, detail) => {
      if (status === 405) {
        return (
          `GitHub would not merge #${number} in ${repo.full} (405)${detail}. It is not mergeable: it may have ` +
          `conflicts, required checks that are failing or pending, a missing approval, or ${method} merges ` +
          "may be disabled for the repository. A repository with a merge queue cannot be merged this way."
        );
      }
      if (status === 409) {
        return (
          `GitHub did not merge #${number} in ${repo.full} (409)${detail}. The branch moved since the commit ` +
          "that was meant to be merged; read the pull request again with pr_read."
        );
      }
      return undefined;
    },
  });

  if (body?.merged !== true) {
    return {
      payload: { summary: `GitHub did not merge #${number} in ${repo.full}${body?.message ? `: ${body.message}` : "."}`, merged: false, ...(body?.message ? { message: body.message } : {}) },
      ok: false,
    };
  }
  return {
    payload: {
      summary: `Merged pull request #${number} in ${repo.full} by ${method}${body.sha ? `, at ${short(body.sha)}` : ""}.`,
      merged: true,
      ...(body.sha ? { sha: body.sha } : {}),
      ...(body.message ? { message: body.message } : {}),
    },
    ok: true,
  };
}
