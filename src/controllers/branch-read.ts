/** Read one branch. */
import { z } from "zod";
import { github } from "../services/api.js";
import { parseRepo, refName, refPath } from "../models/address.js";
import { READ, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";
import { firstLine, short } from "../utils/text.js";
import type { ToolDefinition, ToolResult } from "../models/tool.js";

export const TOOL: ToolDefinition = {
  name: "branch_read",
  description:
    "Read one branch of a GitHub repository: the commit at its tip, that commit's message and date, " +
    "and whether the branch is protected. Use it to find a branch's current sha.",
  inputSchema: {
    repo: repoArg,
    branch: z.string().describe("Branch name, e.g. 'main' or 'feat/login'."),
  },
  outputSchema: {
    name: z.string(),
    sha: z.string(),
    protected: z.boolean(),
    message: z.string().optional(),
    date: z.string().optional(),
  },
  annotations: READ,
};

export async function handle(args: { repo?: unknown; branch?: unknown }): Promise<ToolResult> {
  const repo = parseRepo(args.repo);
  const branch = refName(args.branch, "branch");

  const { body } = await github("GET", `${repo.path}/branches/${refPath(branch)}`, {
    what: `read the branch '${branch}' of ${repo.full}`,
  });
  expectBody(body, "name", "a branch");

  const sha = body.commit?.sha ?? "";
  const message = firstLine(body.commit?.commit?.message);
  const date = body.commit?.commit?.author?.date;
  return {
    payload: {
      summary: `Branch ${body.name} of ${repo.full} is at ${short(sha)}${body.protected ? " and is protected" : ""}.`,
      name: body.name,
      sha,
      protected: Boolean(body.protected),
      ...(message ? { message } : {}),
      ...(date ? { date } : {}),
    },
    ok: true,
  };
}
