/** Open a pull request. */
import { z } from "zod";
import { github, parseRepo, refName } from "../api.js";
import { maybeFlag, rawText, requiredText } from "../args.js";
import { Refused } from "../errors.js";
import { WRITE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

const LOGIN = /^[A-Za-z0-9][A-Za-z0-9_-]{0,38}$/;

/**
 * The head of a pull request: a branch, or `owner:branch` for a branch in a fork.
 *
 * @param {unknown} value
 * @returns {string}
 */
function headRef(value) {
  const text = requiredText(value, "head");
  const colon = text.indexOf(":");
  if (colon === -1) return refName(text, "head");

  const owner = text.slice(0, colon);
  if (!LOGIN.test(owner)) {
    throw new Refused(`\`head\` '${text}' is not 'branch' or 'owner:branch'.`);
  }
  return `${owner}:${refName(text.slice(colon + 1), "head")}`;
}

export const TOOL = {
  name: "pr_open",
  description:
    "Open a pull request in a GitHub repository from a branch into a base branch (default: the " +
    "default branch), optionally as a draft. For a branch in a fork, give the head as 'owner:branch'. " +
    "Fails if there are no commits between them or one is already open.",
  inputSchema: {
    repo: repoArg,
    title: z.string().describe("Pull request title."),
    head: z.string().describe("Branch with the changes, e.g. 'feat/login', or 'owner:branch' for a fork."),
    base: z.string().optional().describe("Branch to merge into. Default: the repository's default branch."),
    body: z.string().optional().describe("Description, in Markdown."),
    draft: z.boolean().optional().describe("true opens it as a draft. Default false."),
  },
  outputSchema: {
    number: z.number(),
    title: z.string(),
    state: z.string(),
    draft: z.boolean(),
    head: z.string(),
    base: z.string(),
    html_url: z.string(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, title?: unknown, head?: unknown, base?: unknown, body?: unknown, draft?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const title = requiredText(args.title, "title");
  const head = headRef(args.head);
  const given = args.base === undefined || args.base === null ? undefined : refName(args.base, "base");
  const body = rawText(args.body, "body");
  const draft = maybeFlag(args.draft, "draft");

  const base =
    given ??
    (await github("GET", repo.path, { what: `find the default branch of ${repo.full}` })).body?.default_branch;
  if (typeof base !== "string") throw new Refused(`Could not find the default branch of ${repo.full}; pass \`base\`.`);

  const { body: created } = await github("POST", `${repo.path}/pulls`, {
    json: { title, head, base, ...(body === undefined ? {} : { body }), ...(draft === undefined ? {} : { draft }) },
    what: `open a pull request from '${head}' into '${base}' in ${repo.full}`,
    explain: (status, detail) => {
      if (status !== 422) return undefined;
      if (/already exists/i.test(detail)) {
        return `GitHub would not open a pull request from '${head}' into '${base}' in ${repo.full} (422)${detail}. One is already open; find it with pr_read.`;
      }
      if (/no commits between/i.test(detail)) {
        return `GitHub would not open a pull request from '${head}' into '${base}' in ${repo.full} (422)${detail}. The branches have no difference to propose.`;
      }
      return undefined;
    },
  });

  expectBody(created, "html_url", "a pull request");
  return {
    payload: {
      summary: `Opened ${created.draft ? "draft " : ""}pull request #${created.number} in ${repo.full}: ${head} into ${base}.`,
      number: created.number,
      title: created.title,
      state: created.state,
      draft: Boolean(created.draft),
      head,
      base,
      html_url: created.html_url,
    },
    ok: true,
  };
}
