/** Comment on a pull request's conversation. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { rawText, requiredInt } from "../models/args.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";

export const TOOL = {
  name: "pr_comment",
  description:
    "Add a comment to a pull request's conversation in a GitHub repository. A conversation comment " +
    "only: it is not attached to a line of the diff and is not a review. Visible to everyone who can " +
    "see the pull request.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Pull request number."),
    body: z.string().describe("Comment text, in Markdown."),
  },
  outputSchema: {
    id: z.number(),
    html_url: z.string(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, number?: unknown, body?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const body = rawText(args.body, "body", { required: true });

  // GitHub keeps a pull request's conversation comments with its issue comments.
  const { body: created } = await github("POST", `${repo.path}/issues/${number}/comments`, {
    json: { body },
    what: `comment on pull request #${number} of ${repo.full}`,
    explain: (status, detail) =>
      status === 403 && /locked/i.test(detail)
        ? `GitHub would not let this comment on #${number} of ${repo.full} (403)${detail}. The conversation is locked.`
        : undefined,
  });

  expectBody(created, "html_url", "a comment");
  return {
    payload: { summary: `Commented on pull request #${number} in ${repo.full}.`, id: created.id, html_url: created.html_url },
    ok: true,
  };
}
