/** List a repository's tags. */
import { z } from "zod";
import { github, pageQuery, paging, parseRepo } from "../api.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../schemas.js";

export const TOOL = {
  name: "tag_get_all",
  description:
    "List the tags of a GitHub repository with the commit each points at. Paged, in the order GitHub " +
    "returns them (newest first for version-like names). Use it to see what has been tagged or " +
    "released.",
  inputSchema: { repo: repoArg, ...pagingArgs },
  outputSchema: {
    items: z.array(z.object({ name: z.string(), sha: z.string() })),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const page = pageQuery(args);

  const { body, headers } = await github("GET", `${repo.path}/tags`, {
    query: page,
    what: `list the tags of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((tag) => ({ name: tag.name, sha: tag.commit?.sha ?? "" }));
  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no tags on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "tag" : "tags"} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
