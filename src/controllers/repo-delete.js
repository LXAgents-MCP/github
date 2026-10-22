/** Delete a repository. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { requiredText } from "../models/args.js";
import { Refused } from "../utils/errors.js";
import { DESTROY, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "repo_delete",
  description:
    "Permanently delete a GitHub repository, with its issues, pull requests and wiki. Destructive " +
    "and not undoable from here: confirm with the owner first. `confirm` must repeat the " +
    "repository's full name. Needs delete rights on the token and admin on the repository.",
  inputSchema: {
    repo: repoArg,
    confirm: z
      .string()
      .describe("The repository's full name again, 'owner/name'. Anything else deletes nothing."),
  },
  outputSchema: {
    deleted: z.boolean(),
    full_name: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, confirm?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const confirm = requiredText(args.confirm, "confirm");

  // The guard is mechanical on purpose: a name typed back cannot be a reflex, a copied flag can.
  if (confirm.toLowerCase() !== repo.full.toLowerCase()) {
    throw new Refused(
      `\`confirm\` must repeat the repository's full name, '${repo.full}', to delete it. ` +
        "Nothing was deleted.",
    );
  }

  await github("DELETE", repo.path, {
    what: `delete the repository ${repo.full}`,
    explain: (status, detail) =>
      status === 403
        ? `GitHub refused to delete ${repo.full} (403)${detail}. Deleting needs admin rights on the ` +
          "repository, and a token that may delete: the `delete_repo` scope for a classic token, " +
          "the Administration permission set to write for a fine-grained one. An organization can " +
          "also forbid its members to delete repositories."
        : undefined,
  });

  return { payload: { summary: `Deleted repository ${repo.full}.`, deleted: true, full_name: repo.full }, ok: true };
}
