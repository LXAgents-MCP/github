/** List a repository's branches. */
import { z } from "zod";
import { github, pageQuery, paging, parseRepo } from "../api.js";
import { maybeFlag } from "../args.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../schemas.js";

export const TOOL = {
  name: "branch_list",
  description:
    "List the branches of a GitHub repository, with the sha at each tip and whether it is protected. " +
    "Paged. Use it to see what branches exist before creating, merging or deleting one.",
  inputSchema: {
    repo: repoArg,
    protected: z
      .boolean()
      .optional()
      .describe("true lists only protected branches, false only unprotected ones. Omit for all."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(z.object({ name: z.string(), sha: z.string(), protected: z.boolean() })),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, protected?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const onlyProtected = maybeFlag(args.protected, "protected");
  const page = pageQuery(args);

  const { body, headers } = await github("GET", `${repo.path}/branches`, {
    query: { protected: onlyProtected, ...page },
    what: `list the branches of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((branch) => ({
    name: branch.name,
    sha: branch.commit?.sha ?? "",
    protected: Boolean(branch.protected),
  }));
  const kind = onlyProtected === undefined ? "" : onlyProtected ? "protected " : "unprotected ";

  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no ${kind}branches on page ${page.page}.`
          : `${items.length} ${kind}${items.length === 1 ? "branch" : "branches"} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
