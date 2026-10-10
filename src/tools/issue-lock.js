/** Lock or unlock an issue's conversation. */
import { z } from "zod";
import { github, parseRepo } from "../api.js";
import { oneOf, optionalFlag, requiredInt } from "../args.js";
import { UPDATE, repoArg } from "../schemas.js";

const REASONS = ["off-topic", "too heated", "resolved", "spam"];

export const TOOL = {
  name: "issue_lock",
  description:
    "Lock an issue or pull request in a GitHub repository so only collaborators can comment, with an " +
    "optional reason, or unlock it. Needs push access.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue or pull request number."),
    locked: z.boolean().optional().describe("true locks (default), false unlocks."),
    reason: z.enum(REASONS).optional().describe("Why it is locked: off-topic, too heated, resolved or spam. Only when locking."),
  },
  outputSchema: {
    number: z.number(),
    locked: z.boolean(),
    reason: z.string().optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, locked?: unknown, reason?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const lock = optionalFlag(args.locked, "locked", true);
  const reason = oneOf(args.reason, "reason", REASONS);
  const path = `${repo.path}/issues/${number}/lock`;

  if (lock) {
    await github("PUT", path, {
      ...(reason === undefined ? {} : { json: { lock_reason: reason } }),
      what: `lock #${number} of ${repo.full}`,
    });
  } else {
    await github("DELETE", path, { what: `unlock #${number} of ${repo.full}` });
  }

  return {
    payload: {
      summary: lock
        ? `Locked #${number} in ${repo.full}${reason ? ` as ${reason}` : ""}.`
        : `Unlocked #${number} in ${repo.full}.`,
      number,
      locked: lock,
      ...(lock && reason ? { reason } : {}),
    },
    ok: true,
  };
}
