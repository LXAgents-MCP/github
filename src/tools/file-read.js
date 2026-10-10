/** Read one text file from a repository. */
import { z } from "zod";
import { filePath, github, parseRepo, refName } from "../api.js";
import { MAX_FILE_CHARS } from "../config.js";
import { contentsPath } from "../contents.js";
import { READ, repoArg } from "../schemas.js";
import { Refused } from "../errors.js";
import { cutText, decodeText, looksBinary } from "../text.js";

export const TOOL = {
  name: "file_read",
  description:
    "Read one text file from a GitHub repository, on the default branch or at a branch, tag or " +
    "commit. Returns its content, size and blob sha. Cut at 25,000 characters; binary files and " +
    "files over 1 MB are refused. Use directory_list to find a path first.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the file inside the repository, e.g. 'src/index.js'."),
    ref: z.string().optional().describe("Branch, tag or commit sha. Default: the default branch."),
  },
  outputSchema: {
    path: z.string(),
    sha: z.string(),
    size: z.number(),
    text: z.string(),
    truncated: z.boolean(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, path?: unknown, ref?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const ref = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");

  const { body } = await github("GET", contentsPath(repo, file), {
    query: { ref },
    what: `read '${file.path}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 403 && /too[_ ]large/i.test(detail)
        ? `'${file.path}' in ${repo.full} is over the 1 MB GitHub returns through this API (403)${detail}. ` +
          "Fetch it with git instead."
        : undefined,
  });

  if (Array.isArray(body)) {
    throw new Refused(`\`path\` '${file.path}' is a directory in ${repo.full}. Use directory_list to see inside it.`);
  }
  if (body?.type === "symlink") {
    throw new Refused(`\`path\` '${file.path}' is a symbolic link to '${body.target ?? "another path"}'. Read that path instead.`);
  }
  if (body?.type === "submodule") {
    throw new Refused(`\`path\` '${file.path}' is a submodule${body.submodule_git_url ? ` of ${body.submodule_git_url}` : ""}, not a file in this repository.`);
  }
  if (!body || body.type !== "file" || body.encoding !== "base64" || typeof body.content !== "string") {
    throw new Refused("GitHub answered, but the body was not a file this server recognizes.");
  }

  const bytes = Buffer.from(body.content, "base64");
  if (looksBinary(bytes)) {
    throw new Refused(`\`path\` '${file.path}' is a binary file (${bytes.length} bytes); this tool reads text.`);
  }
  const [text, truncated] = cutText(decodeText(bytes), MAX_FILE_CHARS);
  const size = Number.isInteger(body.size) ? body.size : bytes.length;

  return {
    payload: {
      summary:
        `Read ${file.path} (${size} bytes) from ${repo.full}${ref ? ` at ${ref}` : ""}` +
        (truncated ? `, cut at ${MAX_FILE_CHARS} characters.` : "."),
      path: file.path,
      sha: body.sha,
      size,
      text,
      truncated,
    },
    ok: true,
  };
}
