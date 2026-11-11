/** Read one repository. */
import { github } from "../services/api.js";
import { parseRepo } from "../models/address.js";
import { READ, repoArg } from "../models/schemas.js";
import { describeRepository, expectBody, repositoryFields, repositoryShape } from "../models/shapes.js";
import type { ToolDefinition, ToolResult } from "../models/tool.js";

export const TOOL: ToolDefinition = {
  name: "repo_read",
  description:
    "Read one GitHub repository: description, visibility, default branch, star, fork and issue " +
    "counts, language, license, topics and parent. Use it to learn about a repository before " +
    "acting on it.",
  inputSchema: { repo: repoArg },
  outputSchema: repositoryShape,
  annotations: READ,
};

export async function handle(args: { repo?: unknown }): Promise<ToolResult> {
  const repo = parseRepo(args.repo);
  const { body } = await github("GET", repo.path, { what: `read the repository ${repo.full}` });

  const fields = repositoryFields(expectBody(body, "full_name", "a repository"));
  return { payload: { summary: describeRepository(fields), ...fields }, ok: true };
}
