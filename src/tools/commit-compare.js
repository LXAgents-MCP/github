/** Compare two refs. */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../api.js";
import { MAX_COMMITS_SHOWN, MAX_FILES_SHOWN } from "../config.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";
import { firstLine } from "../text.js";

export const TOOL = {
  name: "commit_compare",
  description:
    "Compare two branches, tags or commits of a GitHub repository: how far ahead or behind the head " +
    "is of the base, the most recent 50 commits between them, and the changed files with line " +
    "counts. Use it to see what a branch would bring in before merging.",
  inputSchema: {
    repo: repoArg,
    base: z.string().describe("The ref to compare from, e.g. 'main'."),
    head: z.string().describe("The ref to compare, e.g. 'feat/login'."),
  },
  outputSchema: {
    status: z.string(),
    ahead_by: z.number(),
    behind_by: z.number(),
    total_commits: z.number(),
    commits: z.array(z.object({ sha: z.string(), message: z.string(), author: z.string().optional() })),
    file_count: z.number(),
    files: z.array(
      z.object({ filename: z.string(), status: z.string(), additions: z.number(), deletions: z.number() }),
    ),
    truncated: z.boolean(),
    html_url: z.string().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, base?: unknown, head?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const base = refName(args.base, "base");
  const head = refName(args.head, "head");

  const { body } = await github("GET", `${repo.path}/compare/${refPath(base)}...${refPath(head)}`, {
    what: `compare '${head}' with '${base}' in ${repo.full}`,
  });
  expectBody(body, "status", "a comparison");

  const all = Array.isArray(body.commits) ? body.commits : [];
  const listed = Array.isArray(body.files) ? body.files : [];
  // Commits arrive oldest first, and the newest are the ones a reader came for.
  const commits = all.slice(-MAX_COMMITS_SHOWN).map((commit) => ({
    sha: commit.sha,
    message: firstLine(commit.commit?.message),
    ...(commit.commit?.author?.name ? { author: commit.commit.author.name } : {}),
  }));
  const files = listed.slice(0, MAX_FILES_SHOWN).map((file) => ({
    filename: file.filename,
    status: file.status,
    additions: file.additions ?? 0,
    deletions: file.deletions ?? 0,
  }));
  const ahead = body.ahead_by ?? 0;
  const behind = body.behind_by ?? 0;

  const sentence = {
    identical: `${head} and ${base} are identical.`,
    ahead: `${head} is ${ahead} ${ahead === 1 ? "commit" : "commits"} ahead of ${base}.`,
    behind: `${head} is ${behind} ${behind === 1 ? "commit" : "commits"} behind ${base}.`,
    diverged: `${head} has diverged from ${base}: ${ahead} ahead, ${behind} behind.`,
  }[body.status] ?? `${head} compared with ${base}: ${body.status}.`;

  return {
    payload: {
      summary: sentence,
      status: body.status,
      ahead_by: ahead,
      behind_by: behind,
      total_commits: body.total_commits ?? all.length,
      commits,
      file_count: listed.length,
      files,
      truncated: all.length > MAX_COMMITS_SHOWN || listed.length > MAX_FILES_SHOWN,
      ...(body.html_url ? { html_url: body.html_url } : {}),
    },
    ok: true,
  };
}
