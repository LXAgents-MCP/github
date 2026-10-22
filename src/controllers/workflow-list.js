/** List a repository's GitHub Actions workflows. */
import { z } from "zod";
import { github, pageQuery, paging, parseRepo } from "../services/api.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "workflow_list",
  description:
    "List the GitHub Actions workflows of a repository with their id, name, file path and state. Paged. " +
    "Use it to find the workflow to run or to follow; the id or the file name, such as 'ci.yml', identifies it.",
  inputSchema: {
    repo: repoArg,
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(z.object({ id: z.number(), name: z.string(), path: z.string(), state: z.string() })),
    total_count: z.number(),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const page = pageQuery(args);

  const { body, headers } = await github("GET", `${repo.path}/actions/workflows`, {
    query: page,
    what: `list the workflows of ${repo.full}`,
  });

  const items = (Array.isArray(body?.workflows) ? body.workflows : []).map((workflow) => ({
    id: workflow.id,
    name: workflow.name,
    path: workflow.path,
    state: workflow.state,
  }));
  const total = Number.isInteger(body?.total_count) ? body.total_count : items.length;

  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no workflows on page ${page.page}.`
          : `${items.length} of ${total} ${total === 1 ? "workflow" : "workflows"} of ${repo.full}, on page ${page.page}.`,
      items,
      total_count: total,
      ...paging(page, headers),
    },
    ok: true,
  };
}
