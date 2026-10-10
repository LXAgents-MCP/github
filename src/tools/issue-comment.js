/** Comment on an issue. */
import { z } from "zod";
import { github, parseRepo } from "../api.js";
import { rawText, requiredInt } from "../args.js";
import { WRITE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

export const TOOL = {
  name: "issue_comment",
  description:
    "Add a comment to an issue of a GitHub repository. Works on a pull request's number too, as a " +
    "conversation comment. Visible to everyone who can see the issue.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number."),
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

  const { body: created } = await github("POST", `${repo.path}/issues/${number}/comments`, {
    json: { body },
    what: `comment on issue #${number} of ${repo.full}`,
    explain: (status, detail) =>
      status === 403 && /locked/i.test(detail)
        ? `GitHub would not let this comment on #${number} of ${repo.full} (403)${detail}. The conversation is locked.`
        : undefined,
  });

  expectBody(created, "html_url", "a comment");
  return {
    payload: { summary: `Commented on #${number} in ${repo.full}.`, id: created.id, html_url: created.html_url },
    ok: true,
  };
}
