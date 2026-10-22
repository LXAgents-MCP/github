/** List a repository's releases. */
import { z } from "zod";
import { github, pageQuery, paging, parseRepo } from "../services/api.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "release_list",
  description:
    "List the releases of a GitHub repository, newest first, including drafts when the token has " +
    "push access. Paged. Use release_read for one release's notes and assets.",
  inputSchema: { repo: repoArg, ...pagingArgs },
  outputSchema: {
    items: z.array(
      z.object({
        id: z.number(),
        tag: z.string(),
        name: z.string().optional(),
        draft: z.boolean(),
        prerelease: z.boolean(),
        published_at: z.string().optional(),
      }),
    ),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const page = pageQuery(args);

  const { body, headers } = await github("GET", `${repo.path}/releases`, {
    query: page,
    what: `list the releases of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((release) => ({
    id: release.id,
    tag: release.tag_name,
    ...(release.name ? { name: release.name } : {}),
    draft: Boolean(release.draft),
    prerelease: Boolean(release.prerelease),
    ...(release.published_at ? { published_at: release.published_at } : {}),
  }));

  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no releases on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "release" : "releases"} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
