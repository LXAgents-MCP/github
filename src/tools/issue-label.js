/** Change the labels on an issue. */
import { z } from "zod";
import { github, parseRepo } from "../api.js";
import { oneOf, requiredInt, textList } from "../args.js";
import { UPDATE, repoArg } from "../schemas.js";
import { namesOf } from "../shapes.js";

const ACTIONS = ["add", "remove", "set"];

export const TOOL = {
  name: "issue_label",
  description:
    "Add, remove or replace the labels on an issue (or pull request) in a GitHub repository. The result " +
    "lists the labels now on the issue and any requested label that is not as asked.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue or pull request number."),
    labels: z.array(z.string()).describe("Label names, e.g. ['bug', 'good first issue']."),
    action: z
      .enum(ACTIONS)
      .optional()
      .describe("add (default) keeps the current labels and adds these; remove takes these off; set replaces all."),
  },
  outputSchema: {
    number: z.number(),
    labels: z.array(z.string()),
    not_applied: z.array(z.string()).optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, labels?: unknown, action?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const labels = /** @type {string[]} */ (textList(args.labels, "labels", { required: true }));
  const action = oneOf(args.action, "action", ACTIONS, "add");
  const path = `${repo.path}/issues/${number}/labels`;
  const what = `${action} labels on #${number} of ${repo.full}`;

  let now;
  let notApplied;
  if (action === "remove") {
    // One request per label: GitHub removes a single label by name. A 404 means that label is not
    // on the issue (or does not exist), which for "make sure it is off" is not a failure.
    let latest;
    notApplied = [];
    for (const label of labels) {
      const answer = await github("DELETE", `${path}/${encodeURIComponent(label)}`, { allow: [404], what });
      if (answer.status === 404) notApplied.push(label);
      else latest = answer.body;
    }
    now = namesOf(latest ?? (await github("GET", path, { what: `read the labels on #${number} of ${repo.full}` })).body);
  } else {
    const { body } =
      action === "add"
        ? await github("POST", path, { json: { labels }, what })
        : await github("PUT", path, { json: { labels }, what });
    now = namesOf(body);
    notApplied = labels.filter((label) => !now.includes(label));
  }

  return {
    payload: {
      summary:
        `Labels on #${number} in ${repo.full}: ${now.length > 0 ? now.join(", ") : "none"}.` +
        (notApplied.length > 0
          ? ` ${action === "remove" ? "Not on the issue to begin with" : "GitHub did not apply"}: ${notApplied.join(", ")}.`
          : ""),
      number,
      labels: now,
      ...(notApplied.length > 0 ? { not_applied: notApplied } : {}),
    },
    ok: true,
  };
}
