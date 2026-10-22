/** Close a pull request without merging it. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { rawText, requiredInt } from "../models/args.js";
import { Refused } from "../utils/errors.js";
import { UPDATE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";

export const TOOL = {
  name: "pr_close",
  description:
    "Close a pull request in a GitHub repository without merging it, optionally leaving a comment " +
    "first. Its branch is kept. There is no tool to reopen it.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Pull request number."),
    comment: z.string().optional().describe("A comment to post before closing, e.g. why."),
  },
  outputSchema: {
    number: z.number(),
    state: z.string(),
    merged: z.boolean(),
    html_url: z.string(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, comment?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const comment = rawText(args.comment, "comment");
  const commented = comment !== undefined && comment.trim() !== "";

  if (commented) {
    await github("POST", `${repo.path}/issues/${number}/comments`, {
      json: { body: comment },
      what: `comment on pull request #${number} of ${repo.full} before closing it`,
    });
  }

  let closed;
  try {
    closed = (
      await github("PATCH", `${repo.path}/pulls/${number}`, {
        json: { state: "closed" },
        what: `close pull request #${number} of ${repo.full}`,
      })
    ).body;
  } catch (error) {
    if (error instanceof Refused && commented) throw new Refused(`${error.message} The comment had already been posted.`);
    throw error;
  }

  expectBody(closed, "html_url", "a pull request");
  return {
    payload: {
      summary: closed.merged
        ? `Pull request #${number} in ${repo.full} was already merged; nothing to close.`
        : `Closed pull request #${number} in ${repo.full} without merging.`,
      number,
      state: closed.state,
      merged: Boolean(closed.merged),
      html_url: closed.html_url,
    },
    ok: true,
  };
}
