/** Approve a pull request. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { optionalText, rawText, requiredInt } from "../models/args.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";

export const TOOL = {
  name: "pr_approve",
  description:
    "Approve a pull request in a GitHub repository as the token's user, with an optional message. A " +
    "person cannot approve their own pull request. Each call records another approval review.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Pull request number."),
    body: z.string().optional().describe("A message to go with the approval."),
    sha: z
      .string()
      .optional()
      .describe("The head commit being approved; the approval fails if the branch has moved on. Default: the latest."),
  },
  outputSchema: {
    id: z.number(),
    state: z.string(),
    html_url: z.string().optional(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, number?: unknown, body?: unknown, sha?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const body = rawText(args.body, "body");
  const sha = optionalText(args.sha, "sha");

  const { body: review } = await github("POST", `${repo.path}/pulls/${number}/reviews`, {
    json: {
      event: "APPROVE",
      ...(body === undefined || body.trim() === "" ? {} : { body }),
      ...(sha === undefined ? {} : { commit_id: sha }),
    },
    what: `approve pull request #${number} of ${repo.full}`,
    explain: (status, detail) =>
      status === 422 && /own pull request/i.test(detail)
        ? `GitHub would not record the approval on #${number} of ${repo.full} (422)${detail}. A person cannot approve ` +
          "their own pull request; it needs a different reviewer."
        : undefined,
  });

  if (!review || typeof review.state !== "string") expectBody(review, "state", "a review");
  return {
    payload: {
      summary: `Approved pull request #${number} in ${repo.full} (review state ${review.state}).`,
      id: review.id,
      state: review.state,
      ...(review.html_url ? { html_url: review.html_url } : {}),
    },
    ok: true,
  };
}
