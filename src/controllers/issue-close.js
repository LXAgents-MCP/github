/** Close an issue. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { oneOf, rawText, requiredInt } from "../models/args.js";
import { Refused } from "../utils/errors.js";
import { UPDATE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";

const REASONS = ["completed", "not_planned", "duplicate"];

export const TOOL = {
  name: "issue_close",
  description:
    "Close an issue of a GitHub repository, as completed, not planned or a duplicate, optionally " +
    "leaving a comment first. It only closes: there is no tool to reopen. Use pr_close for a pull request.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number."),
    reason: z.enum(REASONS).optional().describe("completed (default), not_planned or duplicate."),
    comment: z.string().optional().describe("A comment to post before closing, e.g. why."),
  },
  outputSchema: {
    number: z.number(),
    state: z.string(),
    state_reason: z.string().optional(),
    html_url: z.string(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, reason?: unknown, comment?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const reason = oneOf(args.reason, "reason", REASONS, "completed");
  const comment = rawText(args.comment, "comment");

  if (comment !== undefined && comment.trim() !== "") {
    await github("POST", `${repo.path}/issues/${number}/comments`, {
      json: { body: comment },
      what: `comment on issue #${number} of ${repo.full} before closing it`,
    });
  }

  let closed;
  try {
    closed = (
      await github("PATCH", `${repo.path}/issues/${number}`, {
        json: { state: "closed", state_reason: reason },
        what: `close issue #${number} of ${repo.full}`,
      })
    ).body;
  } catch (error) {
    if (error instanceof Refused && comment !== undefined && comment.trim() !== "") {
      throw new Refused(`${error.message} The comment had already been posted.`);
    }
    throw error;
  }

  expectBody(closed, "html_url", "an issue");
  return {
    payload: {
      summary: `Closed issue #${number} in ${repo.full} as ${reason.replace("_", " ")}.`,
      number,
      state: closed.state,
      ...(closed.state_reason ? { state_reason: closed.state_reason } : {}),
      html_url: closed.html_url,
    },
    ok: true,
  };
}
