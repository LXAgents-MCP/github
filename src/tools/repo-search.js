/** Search repositories across GitHub. */
import { z } from "zod";
import { github, pageQuery, paging } from "../api.js";
import { oneOf, requiredText } from "../args.js";
import { READ, pagingArgs, pagingOut } from "../schemas.js";

const SORTS = ["stars", "forks", "help-wanted-issues", "updated"];
const ORDERS = ["asc", "desc"];

export const TOOL = {
  name: "repo_search",
  description:
    "Search GitHub repositories with GitHub's search syntax, such as 'org:LXAgents-MCP mcp in:name' " +
    "or 'topic:mcp language:javascript'. Use it to find a repository when the owner is not known. " +
    "GitHub serves at most the first 1000 matches and limits search to 30 requests a minute.",
  inputSchema: {
    query: z
      .string()
      .describe("Keywords and qualifiers, e.g. 'mcp in:name org:LXAgents-MCP' or 'user:octocat archived:false'."),
    sort: z.enum(SORTS).optional().describe("stars, forks, help-wanted-issues or updated. Default: best match."),
    order: z.enum(ORDERS).optional().describe("asc or desc (default). Only applies when sort is given."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(
      z.object({
        full_name: z.string(),
        description: z.string().optional(),
        stars: z.number(),
        language: z.string().optional(),
        url: z.string(),
      }),
    ),
    total_count: z.number(),
    incomplete: z.boolean(),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ query?: unknown, sort?: unknown, order?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const query = requiredText(args.query, "query");
  const sort = oneOf(args.sort, "sort", SORTS);
  const order = oneOf(args.order, "order", ORDERS);
  const page = pageQuery(args);

  const { body, headers } = await github("GET", "/search/repositories", {
    query: { q: query, sort, order: sort ? order : undefined, ...page },
    what: `search repositories for '${query}'`,
  });

  const items = (Array.isArray(body?.items) ? body.items : []).map((repo) => ({
    full_name: repo.full_name,
    ...(repo.description ? { description: repo.description } : {}),
    stars: repo.stargazers_count ?? 0,
    ...(repo.language ? { language: repo.language } : {}),
    url: repo.html_url,
  }));
  const total = Number.isInteger(body?.total_count) ? body.total_count : items.length;

  return {
    payload: {
      summary:
        total === 0
          ? `No repositories match '${query}'.`
          : `${total} ${total === 1 ? "repository matches" : "repositories match"} '${query}'; showing ${items.length}.`,
      items,
      total_count: total,
      incomplete: body?.incomplete_results === true,
      ...paging(page, headers),
    },
    ok: true,
  };
}
