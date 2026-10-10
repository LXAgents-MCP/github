/**
 * GitHub's objects as this server reports them: a schema for a tool's output, and the function
 * that fills it. Kept apart from the tools so every tool that returns a repository (or, later, a
 * commit or an issue) says the same things about it, and a field GitHub renames is changed once.
 */
import { z } from "zod";
import { Refused } from "./errors.js";

/**
 * Check that an answer is the kind of object the tool expected, before reading fields off it.
 *
 * GitHub answers 2xx with a body of the shape documented, and a proxy or an outage can answer
 * 2xx with something else. Reading `body.full_name` off a string would crash; this turns it into
 * a refusal that says what was wrong.
 *
 * @param {any} body
 * @param {string} field A string field the object must have.
 * @param {string} what What the object should have been, for the message.
 * @returns {any} The same body.
 */
export function expectBody(body, field, what) {
  if (!body || typeof body !== "object" || typeof body[field] !== "string") {
    throw new Refused(`GitHub answered, but the body was not ${what} this server recognizes.`);
  }
  return body;
}

/** What every tool that returns a repository reports about it. */
export const repositoryShape = {
  full_name: z.string(),
  description: z.string().optional(),
  html_url: z.string(),
  clone_url: z.string(),
  visibility: z.string(),
  default_branch: z.string().optional(),
  fork: z.boolean(),
  archived: z.boolean(),
  stars: z.number(),
  forks: z.number(),
  open_issues: z.number(),
  language: z.string().optional(),
  license: z.string().optional(),
  topics: z.array(z.string()),
  pushed_at: z.string().optional(),
  parent: z.string().optional(),
};

/**
 * @param {any} repo A repository as GitHub returns it.
 * @returns {z.infer<z.ZodObject<typeof repositoryShape>>}
 */
export function repositoryFields(repo) {
  return {
    full_name: repo.full_name,
    ...(repo.description ? { description: repo.description } : {}),
    html_url: repo.html_url,
    clone_url: repo.clone_url,
    visibility: repo.visibility ?? (repo.private ? "private" : "public"),
    ...(typeof repo.default_branch === "string" ? { default_branch: repo.default_branch } : {}),
    fork: Boolean(repo.fork),
    archived: Boolean(repo.archived),
    stars: repo.stargazers_count ?? 0,
    forks: repo.forks_count ?? 0,
    open_issues: repo.open_issues_count ?? 0,
    ...(repo.language ? { language: repo.language } : {}),
    ...(repo.license?.spdx_id ? { license: repo.license.spdx_id } : {}),
    topics: Array.isArray(repo.topics) ? repo.topics : [],
    ...(repo.pushed_at ? { pushed_at: repo.pushed_at } : {}),
    ...(repo.parent?.full_name ? { parent: repo.parent.full_name } : {}),
  };
}

/**
 * One line about a repository, for a summary.
 *
 * @param {ReturnType<typeof repositoryFields>} fields
 * @returns {string}
 */
export function describeRepository(fields) {
  const kind = fields.fork ? "fork" : "repository";
  const archived = fields.archived ? "archived " : "";
  const branch = fields.default_branch ? `, default branch ${fields.default_branch}` : "";
  return `${fields.full_name} is a ${fields.visibility} ${archived}${kind}${branch}.`;
}
