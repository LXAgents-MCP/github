/** Read one pull request. */
import { z } from "zod";
import { github, parseRepo } from "../api.js";
import { MAX_FILES_SHOWN } from "../config.js";
import { optionalFlag, requiredInt } from "../args.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody, namesOf, reviewersOf } from "../shapes.js";

export const TOOL = {
  name: "pr_read",
  description:
    "Read one pull request of a GitHub repository: state, draft, merged, whether it can be merged, " +
    "branches, size, labels, requested reviewers and description, and with `files: true` the first " +
    "100 changed files. Use issue_read with `comments: true` for the discussion.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Pull request number."),
    files: z.boolean().optional().describe("true also lists the changed files, without their diffs. Default false."),
  },
  outputSchema: {
    number: z.number(),
    title: z.string(),
    state: z.string(),
    draft: z.boolean(),
    merged: z.boolean(),
    mergeable: z.boolean().optional(),
    mergeable_state: z.string().optional(),
    author: z.string().optional(),
    head: z.string(),
    base: z.string(),
    head_sha: z.string(),
    commits: z.number(),
    changed_files: z.number(),
    additions: z.number(),
    deletions: z.number(),
    labels: z.array(z.string()),
    reviewers: z.array(z.string()),
    html_url: z.string(),
    body: z.string().optional(),
    files: z
      .array(z.object({ filename: z.string(), status: z.string(), additions: z.number(), deletions: z.number() }))
      .optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, number?: unknown, files?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const withFiles = optionalFlag(args.files, "files", false);

  const { body } = await github("GET", `${repo.path}/pulls/${number}`, {
    what: `read pull request #${number} of ${repo.full}`,
  });
  expectBody(body, "html_url", "a pull request");

  let files;
  if (withFiles) {
    const answer = await github("GET", `${repo.path}/pulls/${number}/files`, {
      query: { per_page: MAX_FILES_SHOWN },
      what: `list the files of pull request #${number} of ${repo.full}`,
    });
    files = (Array.isArray(answer.body) ? answer.body : []).map((file) => ({
      filename: file.filename,
      status: file.status,
      additions: file.additions ?? 0,
      deletions: file.deletions ?? 0,
    }));
  }

  const state = body.merged ? "merged" : body.state;
  const head = body.head?.ref ?? "";
  const base = body.base?.ref ?? "";
  // `mergeable` is null while GitHub works it out in the background; that is "not known yet", not "no".
  const unknown = body.mergeable === null || body.mergeable === undefined;
  return {
    payload: {
      summary:
        `Pull request #${number} in ${repo.full} is ${body.draft && state === "open" ? "a draft, open" : state}: ` +
        `${body.title} (${head} into ${base}).` +
        (state === "open" && unknown ? " Whether it can be merged is still being computed; ask again in a moment." : ""),
      number,
      title: body.title,
      state: body.state,
      draft: Boolean(body.draft),
      merged: Boolean(body.merged),
      ...(unknown ? {} : { mergeable: Boolean(body.mergeable) }),
      ...(body.mergeable_state ? { mergeable_state: body.mergeable_state } : {}),
      ...(body.user?.login ? { author: body.user.login } : {}),
      head,
      base,
      head_sha: body.head?.sha ?? "",
      commits: body.commits ?? 0,
      changed_files: body.changed_files ?? 0,
      additions: body.additions ?? 0,
      deletions: body.deletions ?? 0,
      labels: namesOf(body.labels),
      reviewers: reviewersOf(body),
      html_url: body.html_url,
      ...(body.body ? { body: body.body } : {}),
      ...(files === undefined ? {} : { files }),
    },
    ok: true,
  };
}
