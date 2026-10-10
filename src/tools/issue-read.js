/** Read one issue, and optionally its discussion. */
import { z } from "zod";
import { github, parseRepo } from "../api.js";
import { optionalFlag, requiredInt } from "../args.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody, issueFields, issueShape } from "../shapes.js";

const COMMENTS_SHOWN = 30;

export const TOOL = {
  name: "issue_read",
  description:
    "Read one issue of a GitHub repository: title, state, author, labels, assignees, milestone and " +
    "body, and with `comments: true` its first 30 comments. Also works on a pull request's number, " +
    "and says when it is one; use pr_read for its branches and diff.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number, e.g. 42."),
    comments: z.boolean().optional().describe("true also returns the first 30 comments. Default false."),
  },
  outputSchema: {
    ...issueShape,
    comments: z
      .array(z.object({ id: z.number(), author: z.string().optional(), created_at: z.string().optional(), body: z.string() }))
      .optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, number?: unknown, comments?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const withComments = optionalFlag(args.comments, "comments", false);

  const { body } = await github("GET", `${repo.path}/issues/${number}`, {
    what: `read issue #${number} of ${repo.full}`,
  });
  expectBody(body, "html_url", "an issue");
  const fields = issueFields(body);

  let comments;
  if (withComments && fields.comment_count > 0) {
    const answer = await github("GET", `${repo.path}/issues/${number}/comments`, {
      query: { per_page: COMMENTS_SHOWN },
      what: `read the comments of issue #${number} of ${repo.full}`,
    });
    comments = (Array.isArray(answer.body) ? answer.body : []).map((comment) => ({
      id: comment.id,
      ...(comment.user?.login ? { author: comment.user.login } : {}),
      ...(comment.created_at ? { created_at: comment.created_at } : {}),
      body: comment.body ?? "",
    }));
  }

  const kind = fields.is_pull_request ? "pull request" : "issue";
  const more = comments !== undefined && fields.comment_count > comments.length ? `; showing the first ${comments.length} comments` : "";
  return {
    payload: {
      summary: `${kind === "issue" ? "Issue" : "Pull request"} #${number} in ${repo.full} is ${fields.state}: ${fields.title}${more}.`,
      ...fields,
      ...(comments === undefined ? {} : { comments }),
    },
    ok: true,
  };
}
