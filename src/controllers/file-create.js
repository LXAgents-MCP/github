/** Create a file in a repository, as a commit. */
import { z } from "zod";
import { filePath, github, parseRepo, refName } from "../services/api.js";
import { rawText, requiredText } from "../models/args.js";
import { contentsPath, encodeContent } from "../services/contents.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { short } from "../utils/text.js";
import { expectBody } from "../models/shapes.js";

export const TOOL = {
  name: "file_create",
  description:
    "Create a new file in a GitHub repository, as one commit on a branch (default: the default " +
    "branch). Fails if the file already exists; use file_update to change it. Content up to 1 MB.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the new file, e.g. 'docs/notes.md'. Missing folders are created."),
    content: z.string().describe("The file's full content. An empty string creates an empty file."),
    message: z.string().describe("Commit message, e.g. 'docs: add notes'."),
    branch: z.string().optional().describe("Branch to commit to. Default: the default branch."),
  },
  outputSchema: {
    path: z.string(),
    sha: z.string(),
    commit_sha: z.string(),
    branch: z.string().optional(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, path?: unknown, content?: unknown, message?: unknown, branch?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const content = rawText(args.content, "content", { required: true, allowEmpty: true });
  const message = requiredText(args.message, "message");
  const branch = args.branch === undefined || args.branch === null ? undefined : refName(args.branch, "branch");

  const { body } = await github("PUT", contentsPath(repo, file), {
    json: { message, content: encodeContent(content), ...(branch === undefined ? {} : { branch }) },
    what: `create '${file.path}' in ${repo.full}`,
    explain: (status, detail) => {
      if (status === 422 && /\bsha\b/i.test(detail)) {
        return (
          `GitHub could not create '${file.path}' in ${repo.full} (422)${detail}. The file already exists ` +
          `${branch ? `on ${branch}` : "on that branch"}; use file_update to change it.`
        );
      }
      if (status === 409) {
        return `GitHub could not create '${file.path}' in ${repo.full} (409)${detail}. The branch moved while the commit was made; try again.`;
      }
      return undefined;
    },
  });

  expectBody(body?.commit, "sha", "a commit");
  const blob = body.content?.sha ?? "";
  return {
    payload: {
      summary: `Created ${file.path} in ${repo.full} on ${branch ?? "the default branch"} (commit ${short(body.commit.sha)}).`,
      path: file.path,
      sha: blob,
      commit_sha: body.commit.sha,
      ...(branch === undefined ? {} : { branch }),
    },
    ok: true,
  };
}
