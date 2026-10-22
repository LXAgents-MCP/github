/** Read one repository. */
import { github, parseRepo } from "../services/api.js";
import { READ, repoArg } from "../models/schemas.js";
import { describeRepository, expectBody, repositoryFields, repositoryShape } from "../models/shapes.js";

export const TOOL = {
  name: "repo_read",
  description:
    "Read one GitHub repository: description, visibility, default branch, star, fork and issue " +
    "counts, language, license, topics and parent. Use it to learn about a repository before " +
    "acting on it.",
  inputSchema: { repo: repoArg },
  outputSchema: repositoryShape,
  annotations: READ,
};

/** @param {{ repo?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const { body } = await github("GET", repo.path, { what: `read the repository ${repo.full}` });

  const fields = repositoryFields(expectBody(body, "full_name", "a repository"));
  return { payload: { summary: describeRepository(fields), ...fields }, ok: true };
}
