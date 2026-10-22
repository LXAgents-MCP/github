/** List what is in a directory of a repository. */
import { z } from "zod";
import { filePath, github, parseRepo, refName } from "../services/api.js";
import { MAX_LIST_ENTRIES } from "../config/config.js";
import { contentsPath } from "../services/contents.js";
import { Refused } from "../utils/errors.js";
import { READ, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "directory_list",
  description:
    "List the files and folders in one directory of a GitHub repository, on the default branch or " +
    "at a branch, tag or commit. Not recursive. Omit the path for the repository root. Use it to " +
    "find a path before file_read.",
  inputSchema: {
    repo: repoArg,
    path: z.string().optional().describe("Directory inside the repository, e.g. 'src/tools'. Omit for the root."),
    ref: z.string().optional().describe("Branch, tag or commit sha. Default: the default branch."),
  },
  outputSchema: {
    path: z.string(),
    items: z.array(z.object({ name: z.string(), type: z.string(), size: z.number() })),
    count: z.number(),
    truncated: z.boolean(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, path?: unknown, ref?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const dir = filePath(args.path, "path", { allowEmpty: true });
  const ref = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");
  const where = dir.path === "" ? "the root" : `'${dir.path}'`;

  const { body } = await github("GET", contentsPath(repo, dir), {
    query: { ref },
    what: `list ${where} of ${repo.full}`,
  });

  if (!Array.isArray(body)) {
    throw new Refused(`\`path\` '${dir.path}' is a file in ${repo.full}, not a directory. Use file_read to read it.`);
  }

  const items = body.slice(0, MAX_LIST_ENTRIES).map((entry) => ({
    name: entry.name,
    type: entry.type,
    size: Number.isInteger(entry.size) ? entry.size : 0,
  }));
  const truncated = body.length > MAX_LIST_ENTRIES;

  return {
    payload: {
      summary:
        `${body.length} ${body.length === 1 ? "entry" : "entries"} in ${where} of ${repo.full}` +
        (ref ? ` at ${ref}` : "") +
        (truncated ? `; showing the first ${MAX_LIST_ENTRIES}.` : "."),
      path: dir.path,
      items,
      count: body.length,
      truncated,
    },
    ok: true,
  };
}
