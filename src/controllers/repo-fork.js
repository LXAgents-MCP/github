/** Fork a repository. */
import { z } from "zod";
import { REPO_NAME, github, parseRepo } from "../services/api.js";
import { maybeFlag, optionalText } from "../models/args.js";
import { Refused } from "../utils/errors.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { expectBody, repositoryFields, repositoryShape } from "../models/shapes.js";

export const TOOL = {
  name: "repo_fork",
  description:
    "Fork a GitHub repository into the token owner's account or an organization. GitHub copies " +
    "it in the background, so the fork may take a moment to be readable. Use it to work on a " +
    "repository the owner cannot push to.",
  inputSchema: {
    repo: repoArg,
    organization: z.string().optional().describe("Organization to fork into. Omit to fork into the token owner's account."),
    name: z.string().optional().describe("Name for the fork. Omit to keep the source's name."),
    default_branch_only: z
      .boolean()
      .optional()
      .describe("true copies only the default branch. Default false."),
  },
  outputSchema: repositoryShape,
  annotations: WRITE,
};

/** @param {{ repo?: unknown, organization?: unknown, name?: unknown, default_branch_only?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const organization = optionalText(args.organization, "organization");
  const name = optionalText(args.name, "name");
  if (name !== undefined && (!REPO_NAME.test(name) || name === "." || name === "..")) {
    throw new Refused(`\`name\` '${name}' is not a valid GitHub repository name.`);
  }
  const defaultBranchOnly = maybeFlag(args.default_branch_only, "default_branch_only");

  const json = Object.fromEntries(
    Object.entries({ organization, name, default_branch_only: defaultBranchOnly }).filter(([, v]) => v !== undefined),
  );
  const { body } = await github("POST", `${repo.path}/forks`, {
    json,
    what: `fork ${repo.full}`,
  });

  const fields = repositoryFields(expectBody(body, "full_name", "a repository"));
  return {
    payload: {
      summary:
        `Forking ${repo.full} into ${fields.full_name} has started. GitHub finishes the copy in the ` +
        "background, so give it a moment before reading it.",
      ...fields,
    },
    ok: true,
  };
}
