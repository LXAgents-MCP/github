/** Change who an issue is assigned to. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { oneOf, requiredInt, textList } from "../models/args.js";
import { UPDATE, repoArg } from "../models/schemas.js";
import { expectBody, namesOf } from "../models/shapes.js";

const ACTIONS = ["add", "remove", "set"];

export const TOOL = {
  name: "issue_assign",
  description:
    "Add, remove or replace the assignees of an issue (or pull request) in a GitHub repository. GitHub " +
    "ignores people who cannot be assigned, such as non-collaborators; the result lists any it did not apply.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue or pull request number."),
    assignees: z.array(z.string()).describe("GitHub logins. At most 10 when adding."),
    action: z
      .enum(ACTIONS)
      .optional()
      .describe("add (default) keeps the current assignees and adds these; remove takes these off; set replaces all."),
  },
  outputSchema: {
    number: z.number(),
    assignees: z.array(z.string()),
    not_applied: z.array(z.string()).optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, assignees?: unknown, action?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const assignees = /** @type {string[]} */ (textList(args.assignees, "assignees", { required: true, max: 10 }));
  const action = oneOf(args.action, "action", ACTIONS, "add");
  const path = `${repo.path}/issues/${number}`;
  const what = `${action} assignees on #${number} of ${repo.full}`;

  const { body } =
    action === "add"
      ? await github("POST", `${path}/assignees`, { json: { assignees }, what })
      : action === "remove"
        ? await github("DELETE", `${path}/assignees`, { json: { assignees }, what })
        : await github("PATCH", path, { json: { assignees }, what });

  expectBody(body, "html_url", "an issue");
  const now = namesOf(body.assignees);
  const notApplied = action === "remove" ? assignees.filter((login) => now.includes(login)) : assignees.filter((login) => !now.includes(login));

  return {
    payload: {
      summary:
        `Assignees of #${number} in ${repo.full}: ${now.length > 0 ? now.join(", ") : "none"}.` +
        (notApplied.length > 0
          ? ` GitHub did not ${action === "remove" ? "remove" : "assign"}: ${notApplied.join(", ")}. ` +
            (action === "remove" ? "" : "They may not be collaborators, or the token may lack push access.")
          : ""),
      number,
      assignees: now,
      ...(notApplied.length > 0 ? { not_applied: notApplied } : {}),
    },
    ok: true,
  };
}
