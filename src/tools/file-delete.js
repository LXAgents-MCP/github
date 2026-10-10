/** Delete a file from a repository, as a commit. */
import { z } from "zod";
import { filePath, github, parseRepo, refName } from "../api.js";
import { optionalText, requiredText } from "../args.js";
import { blobSha, contentsPath } from "../contents.js";
import { DESTROY, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";
import { short } from "../text.js";

export const TOOL = {
  name: "file_delete",
  description:
    "Delete one file from a GitHub repository, as one commit on a branch (default: the default " +
    "branch). Destructive: the file stays in history but leaves the branch, so confirm with the " +
    "owner first. Reads the file's current sha first unless one is passed.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the file to delete."),
    message: z.string().describe("Commit message."),
    branch: z.string().optional().describe("Branch to commit to. Default: the default branch."),
    sha: z.string().optional().describe("Blob sha of the version being deleted (from file_read)."),
  },
  outputSchema: {
    deleted: z.boolean(),
    path: z.string(),
    commit_sha: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, path?: unknown, message?: unknown, branch?: unknown, sha?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const message = requiredText(args.message, "message");
  const branch = args.branch === undefined || args.branch === null ? undefined : refName(args.branch, "branch");
  const given = optionalText(args.sha, "sha");

  const sha = given ?? (await blobSha(repo, file, branch, "There is nothing to delete."));
  const { body } = await github("DELETE", contentsPath(repo, file), {
    json: { message, sha, ...(branch === undefined ? {} : { branch }) },
    what: `delete '${file.path}' from ${repo.full}`,
    explain: (status, detail) =>
      status === 409 || (status === 422 && /\bsha\b/i.test(detail))
        ? `GitHub could not delete '${file.path}' from ${repo.full} (${status})${detail}. The file changed since its ` +
          "sha was read; read it again with file_read before deleting."
        : undefined,
  });

  expectBody(body?.commit, "sha", "a commit");
  return {
    payload: {
      summary: `Deleted ${file.path} from ${repo.full} on ${branch ?? "the default branch"} (commit ${short(body.commit.sha)}).`,
      deleted: true,
      path: file.path,
      commit_sha: body.commit.sha,
    },
    ok: true,
  };
}
