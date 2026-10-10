/**
 * Create a repository on GitHub.
 *
 * Moved from the `cli` server, where it was `create_github_repo`. The arguments, the defaults
 * and the wording of its refusals are unchanged.
 */
import { z } from "zod";
import { REPO_NAME, github } from "../api.js";
import { optionalFlag, optionalText, requiredText } from "../args.js";
import { Refused } from "../errors.js";
import { WRITE } from "../schemas.js";

// GitHub's own rule for an organization login: letters, digits and '-', up to 39 characters.
// A repository name's rule is shared with the other tools, in `src/api.js`.
const ORG_NAME = /^[A-Za-z0-9-]{1,39}$/;

export const TOOL = {
  name: "repo_create",
  description:
    "Create a new, empty repository on GitHub and return its URLs. Use this when the owner " +
    "asks to start a new GitHub repository, in their own account or in an organization. " +
    "Private unless told otherwise. It only creates: it does not clone, push to, change or " +
    "delete anything. The token is read from the LXAGENTS_MCP_GITHUB_API_KEY environment " +
    "variable of the server, never from an argument.",
  inputSchema: {
    name: z
      .string()
      .describe(
        "Repository name: letters, digits, '-', '_' and '.', at most 100 characters. " +
          "Example: 'my-new-repo'.",
      ),
    org: z
      .string()
      .optional()
      .describe(
        "Organization to create it in. Omit to create it in the personal account that " +
          "owns the token. Example: 'LXAgents-MCP'.",
      ),
    description: z.string().optional().describe("One-line description shown on the repository."),
    private: z
      .boolean()
      .optional()
      .describe("Default true. Pass false only when the owner asked for a public repository."),
    auto_init: z
      .boolean()
      .optional()
      .describe(
        "Default true: create an initial commit with a README, so the repository can be " +
          "cloned straight away. Pass false for a truly empty repository.",
      ),
  },
  outputSchema: {
    name: z.string(),
    full_name: z.string(),
    html_url: z.string(),
    clone_url: z.string(),
    visibility: z.string(),
    default_branch: z.string().optional(),
  },
  annotations: WRITE,
};

/**
 * The wording of the statuses a create can meet, which names the next step for each.
 *
 * @param {number} status
 * @param {string} detail GitHub's own message, as a suffix.
 * @param {{ name: string, org: string | undefined, where: string }} request
 * @returns {string | undefined} `undefined` leaves a 401 to the general wording.
 */
function explain(status, detail, { name, org, where }) {
  switch (status) {
    case 401:
      return undefined;
    case 403:
      return (
        `GitHub refused the request (403)${detail}. The token must be allowed to create ` +
        `repositories ${where}: a classic token needs the \`repo\` scope, a fine-grained ` +
        "token needs the Administration permission set to write. If the message mentions a " +
        "rate limit, wait and try again."
      );
    case 404:
      return org
        ? `GitHub could not find the organization '${org}' (404), or the token cannot see ` +
            "it. Check the spelling, or omit `org` to create the repository in the token " +
            "owner's account."
        : `GitHub answered 404${detail}.`;
    case 422:
      return (
        `GitHub could not create '${name}' (422)${detail}. The most common cause is that a ` +
        `repository with that name already exists ${where}; pick another name.`
      );
    default:
      return `GitHub answered ${status}${detail}.`;
  }
}

/**
 * @param {{ name?: unknown, org?: unknown, description?: unknown, private?: unknown, auto_init?: unknown }} args
 * @returns {Promise<{ payload: object, ok: boolean }>}
 */
export async function handle(args) {
  const name = requiredText(args.name, "name");
  if (!REPO_NAME.test(name) || name === "." || name === "..") {
    throw new Refused(
      `\`name\` '${name}' is not a valid GitHub repository name. Use letters, digits, '-', ` +
        "'_' and '.', at most 100 characters.",
    );
  }

  const org = optionalText(args.org, "org");
  if (org !== undefined && !ORG_NAME.test(org)) {
    throw new Refused(
      `\`org\` '${org}' is not a valid GitHub organization name. Use letters, digits and '-', ` +
        "at most 39 characters; omit it to create the repository in the token owner's account.",
    );
  }

  const description = optionalText(args.description, "description");
  const isPrivate = optionalFlag(args.private, "private", true);
  const autoInit = optionalFlag(args.auto_init, "auto_init", true);

  const where = org ? `in '${org}'` : "in the token owner's account";
  const { status, body } = await github("POST", org ? `/orgs/${encodeURIComponent(org)}/repos` : "/user/repos", {
    json: {
      name,
      private: isPrivate,
      auto_init: autoInit,
      ...(description === undefined ? {} : { description }),
    },
    what: `create the repository '${name}' ${where}`,
    explain: (code, detail) => explain(code, detail, { name, org, where }),
  });

  if (!body || typeof body !== "object" || typeof body.html_url !== "string") {
    throw new Refused(`GitHub answered ${status}, but the body was not a repository this server recognizes.`);
  }

  const visibility = body.visibility ?? (body.private ? "private" : "public");
  return {
    payload: {
      summary: `Created ${visibility} repository ${body.full_name} on GitHub.`,
      name: body.name,
      full_name: body.full_name,
      html_url: body.html_url,
      clone_url: body.clone_url,
      visibility,
      ...(typeof body.default_branch === "string" ? { default_branch: body.default_branch } : {}),
    },
    ok: true,
  };
}
