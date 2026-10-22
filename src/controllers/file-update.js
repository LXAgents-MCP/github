/** Replace a file's content in a repository, as a commit. */
import { z } from "zod";
import { filePath, github, parseRepo, refName } from "../services/api.js";
import { optionalText, rawText, requiredText } from "../models/args.js";
import { blobSha, contentsPath, encodeContent } from "../services/contents.js";
import { UPDATE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";
import { short } from "../utils/text.js";

export const TOOL = {
  name: "file_update",
  description:
    "Replace the content of an existing file in a GitHub repository, as one commit on a branch " +
    "(default: the default branch). Reads the file's current sha first unless one is passed. Use " +
    "file_create for a new file.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the existing file."),
    content: z.string().describe("The file's new full content - not a patch."),
    message: z.string().describe("Commit message."),
    branch: z.string().optional().describe("Branch to commit to. Default: the default branch."),
    sha: z
      .string()
      .optional()
      .describe("Blob sha of the version being replaced (from file_read). Pass it to fail if the file changed since."),
  },
  outputSchema: {
    path: z.string(),
    sha: z.string(),
    commit_sha: z.string(),
    branch: z.string().optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, path?: unknown, content?: unknown, message?: unknown, branch?: unknown, sha?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const content = rawText(args.content, "content", { required: true, allowEmpty: true });
  const message = requiredText(args.message, "message");
  const branch = args.branch === undefined || args.branch === null ? undefined : refName(args.branch, "branch");
  const given = optionalText(args.sha, "sha");

  const sha = given ?? (await blobSha(repo, file, branch, "Use file_create to make it."));
  const { body } = await github("PUT", contentsPath(repo, file), {
    json: { message, content: encodeContent(content), sha, ...(branch === undefined ? {} : { branch }) },
    what: `update '${file.path}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 409 || (status === 422 && /\bsha\b/i.test(detail))
        ? `GitHub could not update '${file.path}' in ${repo.full} (${status})${detail}. The file changed since its ` +
          "sha was read; read it again with file_read and apply the change to that version."
        : undefined,
  });

  expectBody(body?.commit, "sha", "a commit");
  return {
    payload: {
      summary: `Updated ${file.path} in ${repo.full} on ${branch ?? "the default branch"} (commit ${short(body.commit.sha)}).`,
      path: file.path,
      sha: body.content?.sha ?? "",
      commit_sha: body.commit.sha,
      ...(branch === undefined ? {} : { branch }),
    },
    ok: true,
  };
}
