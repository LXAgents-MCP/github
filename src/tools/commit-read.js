/** Read one commit. */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../api.js";
import { optionalFlag } from "../args.js";
import { MAX_DIFF_CHARS, MAX_FILES_SHOWN } from "../config.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";
import { firstLine, head, short } from "../text.js";

export const TOOL = {
  name: "commit_read",
  description:
    "Read one commit of a GitHub repository: its message, author, date, parents, the files it changed " +
    "with their line counts, and optionally its diff (cut at 20,000 characters). Use it to see what a " +
    "commit did. Defaults to the tip of the default branch.",
  inputSchema: {
    repo: repoArg,
    ref: z.string().optional().describe("Commit sha, branch or tag. Default: the default branch's tip."),
    diff: z.boolean().optional().describe("true also returns the commit's diff. Default false."),
  },
  outputSchema: {
    sha: z.string(),
    message: z.string(),
    author: z.string().optional(),
    date: z.string().optional(),
    parents: z.array(z.string()),
    additions: z.number(),
    deletions: z.number(),
    file_count: z.number(),
    files: z.array(
      z.object({ filename: z.string(), status: z.string(), additions: z.number(), deletions: z.number() }),
    ),
    diff: z.string().optional(),
    truncated: z.boolean(),
    html_url: z.string().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, ref?: unknown, diff?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const ref = args.ref === undefined || args.ref === null ? "HEAD" : refName(args.ref, "ref");
  const wantDiff = optionalFlag(args.diff, "diff", false);
  const path = `${repo.path}/commits/${refPath(ref)}`;

  const { body } = await github("GET", path, { what: `read the commit '${ref}' of ${repo.full}` });
  expectBody(body, "sha", "a commit");

  const listed = Array.isArray(body.files) ? body.files : [];
  const files = listed.slice(0, MAX_FILES_SHOWN).map((file) => ({
    filename: file.filename,
    status: file.status,
    additions: file.additions ?? 0,
    deletions: file.deletions ?? 0,
  }));
  let truncated = listed.length > MAX_FILES_SHOWN;

  let diff;
  if (wantDiff) {
    const answer = await github("GET", path, {
      accept: "application/vnd.github.diff",
      raw: true,
      what: `read the diff of the commit '${ref}' of ${repo.full}`,
      explain: (status) =>
        status >= 500
          ? `GitHub could not produce the diff of '${ref}' in ${repo.full} (${status}). Very large diffs time out; ` +
            "read the changed files from the commit without the diff, or compare a smaller range."
          : undefined,
    });
    const [shown, cut] = head(typeof answer.body === "string" ? answer.body : "", MAX_DIFF_CHARS);
    diff = shown;
    truncated = truncated || cut;
  }

  const subject = firstLine(body.commit?.message);
  const author = body.commit?.author?.name;
  return {
    payload: {
      summary: `Commit ${short(body.sha)}${author ? ` by ${author}` : ""}: ${subject || "(no message)"}`,
      sha: body.sha,
      message: body.commit?.message ?? "",
      ...(author ? { author } : {}),
      ...(body.commit?.author?.date ? { date: body.commit.author.date } : {}),
      parents: (Array.isArray(body.parents) ? body.parents : []).map((parent) => parent.sha),
      additions: body.stats?.additions ?? 0,
      deletions: body.stats?.deletions ?? 0,
      file_count: listed.length,
      files,
      ...(diff === undefined ? {} : { diff }),
      truncated,
      ...(body.html_url ? { html_url: body.html_url } : {}),
    },
    ok: true,
  };
}
