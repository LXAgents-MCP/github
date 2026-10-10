/** Change a repository's settings. */
import { z } from "zod";
import { REPO_NAME, github, parseRepo, refName } from "../api.js";
import { maybeFlag, optionalText, rawText } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody, repositoryFields, repositoryShape } from "../shapes.js";

export const TOOL = {
  name: "repo_update",
  description:
    "Change a GitHub repository's settings: rename it, set its description or homepage, make it " +
    "private or public, change the default branch, archive or unarchive it, or switch issues on " +
    "or off. Pass only what should change; at least one setting is required.",
  inputSchema: {
    repo: repoArg,
    name: z.string().optional().describe("New repository name. Renaming redirects the old name."),
    description: z.string().optional().describe("New description. Pass an empty string to clear it."),
    homepage: z.string().optional().describe("New homepage URL. Pass an empty string to clear it."),
    private: z.boolean().optional().describe("true makes the repository private, false makes it public."),
    default_branch: z.string().optional().describe("Branch to make the default. It must already exist."),
    archived: z.boolean().optional().describe("true archives the repository (read-only), false unarchives it."),
    has_issues: z.boolean().optional().describe("Whether the Issues tab is enabled."),
  },
  outputSchema: repositoryShape,
  annotations: UPDATE,
};

/**
 * @param {{ repo?: unknown, name?: unknown, description?: unknown, homepage?: unknown,
 *   private?: unknown, default_branch?: unknown, archived?: unknown, has_issues?: unknown }} args
 */
export async function handle(args) {
  const repo = parseRepo(args.repo);

  const name = optionalText(args.name, "name");
  if (name !== undefined && (!REPO_NAME.test(name) || name === "." || name === "..")) {
    throw new Refused(
      `\`name\` '${name}' is not a valid GitHub repository name. Use letters, digits, '-', '_' and '.', ` +
        "at most 100 characters.",
    );
  }
  const description = rawText(args.description, "description");
  const homepage = rawText(args.homepage, "homepage");
  const isPrivate = maybeFlag(args.private, "private");
  const defaultBranch = args.default_branch === undefined ? undefined : refName(args.default_branch, "default_branch");
  const archived = maybeFlag(args.archived, "archived");
  const hasIssues = maybeFlag(args.has_issues, "has_issues");

  const changes = {
    name,
    description,
    homepage,
    private: isPrivate,
    default_branch: defaultBranch,
    archived,
    has_issues: hasIssues,
  };
  const json = Object.fromEntries(Object.entries(changes).filter(([, value]) => value !== undefined));
  if (Object.keys(json).length === 0) {
    throw new Refused(
      "Nothing to change: pass at least one of name, description, homepage, private, default_branch, " +
        "archived or has_issues.",
    );
  }

  const { body } = await github("PATCH", repo.path, {
    json,
    what: `update the repository ${repo.full}`,
  });

  const fields = repositoryFields(expectBody(body, "full_name", "a repository"));
  return {
    payload: { summary: `Updated ${fields.full_name}: ${Object.keys(json).join(", ")}.`, ...fields },
    ok: true,
  };
}
