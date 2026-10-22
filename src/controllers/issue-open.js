/** Open an issue. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { optionalInt, rawText, requiredText, textList } from "../models/args.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { expectBody, namesOf } from "../models/shapes.js";

export const TOOL = {
  name: "issue_open",
  description:
    "Open a new issue in a GitHub repository, with optional body, labels, assignees and milestone. " +
    "GitHub ignores labels, assignees and milestone from a token without push access; the result " +
    "lists what was actually applied.",
  inputSchema: {
    repo: repoArg,
    title: z.string().describe("Issue title."),
    body: z.string().optional().describe("Issue body, in Markdown."),
    labels: z.array(z.string()).optional().describe("Label names to apply. They must already exist."),
    assignees: z.array(z.string()).optional().describe("GitHub logins to assign."),
    milestone: z.number().int().optional().describe("Milestone number (not its title)."),
  },
  outputSchema: {
    number: z.number(),
    title: z.string(),
    state: z.string(),
    html_url: z.string(),
    labels: z.array(z.string()),
    assignees: z.array(z.string()),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, title?: unknown, body?: unknown, labels?: unknown, assignees?: unknown, milestone?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const title = requiredText(args.title, "title");
  const body = rawText(args.body, "body");
  const labels = textList(args.labels, "labels");
  const assignees = textList(args.assignees, "assignees");
  const milestone = optionalInt(args.milestone, "milestone", { min: 1 });

  const { body: created } = await github("POST", `${repo.path}/issues`, {
    json: {
      title,
      ...(body === undefined ? {} : { body }),
      ...(labels === undefined ? {} : { labels }),
      ...(assignees === undefined ? {} : { assignees }),
      ...(milestone === undefined ? {} : { milestone }),
    },
    what: `open an issue in ${repo.full}`,
    explain: (status, detail) =>
      status === 410
        ? `Issues are disabled in ${repo.full} (410)${detail}. Turn them on with repo_update (has_issues).`
        : undefined,
  });

  expectBody(created, "html_url", "an issue");
  const applied = { labels: namesOf(created.labels), assignees: namesOf(created.assignees) };
  const dropped = [
    ...(labels ?? []).filter((name) => !applied.labels.includes(name)).map((name) => `label ${name}`),
    ...(assignees ?? []).filter((login) => !applied.assignees.includes(login)).map((login) => `assignee ${login}`),
  ];
  return {
    payload: {
      summary:
        `Opened issue #${created.number} in ${repo.full}: ${created.title}.` +
        (dropped.length > 0 ? ` GitHub did not apply: ${dropped.join(", ")}. The token may lack push access, or the name does not exist.` : ""),
      number: created.number,
      title: created.title,
      state: created.state,
      html_url: created.html_url,
      ...applied,
    },
    ok: true,
  };
}
