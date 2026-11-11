/**
 * GitHub's objects as this server reports them: a schema for a tool's output, and the function
 * that fills it. Kept apart from the tools so every tool that returns a repository (or, later, a
 * commit or an issue) says the same things about it, and a field GitHub renames is changed once.
 */
import { z } from "zod";
import { Refused } from "../utils/errors.js";
import type { GitHubObject } from "./github-object.js";

/**
 * Check that an answer is the kind of object the tool expected, before reading fields off it.
 *
 * GitHub answers 2xx with a body of the shape documented, and a proxy or an outage can answer
 * 2xx with something else. Reading `body.full_name` off a string would crash; this turns it into
 * a refusal that says what was wrong.
 *
 * @param field A string field the object must have.
 * @param what What the object should have been, for the message.
 * @returns The same body.
 */
export function expectBody(body: unknown, field: string, what: string): GitHubObject {
  if (!body || typeof body !== "object" || typeof (body as GitHubObject)[field] !== "string") {
    throw new Refused(`GitHub answered, but the body was not ${what} this server recognizes.`);
  }
  return body as GitHubObject;
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
 * @param repo A repository as GitHub returns it.
 */
export function repositoryFields(repo: GitHubObject): z.infer<z.ZodObject<typeof repositoryShape>> {
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
 */
export function describeRepository(fields: ReturnType<typeof repositoryFields>): string {
  const kind = fields.fork ? "fork" : "repository";
  const archived = fields.archived ? "archived " : "";
  const branch = fields.default_branch ? `, default branch ${fields.default_branch}` : "";
  return `${fields.full_name} is a ${fields.visibility} ${archived}${kind}${branch}.`;
}

/** What every tool that returns a release reports about it. */
export const releaseShape = {
  id: z.number(),
  tag: z.string(),
  name: z.string().optional(),
  draft: z.boolean(),
  prerelease: z.boolean(),
  html_url: z.string(),
  published_at: z.string().optional(),
  body: z.string().optional(),
  assets: z.array(z.object({ name: z.string(), size: z.number(), downloads: z.number() })),
};

/**
 * @param release A release as GitHub returns it.
 */
export function releaseFields(release: GitHubObject): z.infer<z.ZodObject<typeof releaseShape>> {
  return {
    id: release.id,
    tag: release.tag_name,
    ...(release.name ? { name: release.name } : {}),
    draft: Boolean(release.draft),
    prerelease: Boolean(release.prerelease),
    html_url: release.html_url,
    ...(release.published_at ? { published_at: release.published_at } : {}),
    ...(release.body ? { body: release.body } : {}),
    assets: (Array.isArray(release.assets) ? release.assets : []).map((asset: GitHubObject) => ({
      name: asset.name,
      size: asset.size ?? 0,
      downloads: asset.download_count ?? 0,
    })),
  };
}

/**
 * One line about a release, for a summary.
 */
export function describeRelease(fields: ReturnType<typeof releaseFields>): string {
  const kind = fields.draft ? "draft release" : fields.prerelease ? "prerelease" : "release";
  return `${kind} ${fields.name ?? fields.tag} (tag ${fields.tag})`;
}

/** The names in a list GitHub returns as strings or as objects with a `name` or a `login`. */
export function namesOf(list: unknown): string[] {
  return (Array.isArray(list) ? list : [])
    .map((entry: string | GitHubObject | null): unknown => (typeof entry === "string" ? entry : (entry?.name ?? entry?.login)))
    .filter((name): name is string => Boolean(name));
}

/** What `issue_read` reports about an issue - and, because a pull request is an issue, about that. */
export const issueShape = {
  number: z.number(),
  title: z.string(),
  state: z.string(),
  state_reason: z.string().optional(),
  author: z.string().optional(),
  labels: z.array(z.string()),
  assignees: z.array(z.string()),
  milestone: z.string().optional(),
  comment_count: z.number(),
  is_pull_request: z.boolean(),
  html_url: z.string(),
  created_at: z.string().optional(),
  updated_at: z.string().optional(),
  closed_at: z.string().optional(),
  body: z.string().optional(),
};

/**
 * @param issue An issue as GitHub returns it.
 */
export function issueFields(issue: GitHubObject): z.infer<z.ZodObject<typeof issueShape>> {
  return {
    number: issue.number,
    title: issue.title,
    state: issue.state,
    ...(issue.state_reason ? { state_reason: issue.state_reason } : {}),
    ...(issue.user?.login ? { author: issue.user.login } : {}),
    labels: namesOf(issue.labels),
    assignees: namesOf(issue.assignees),
    ...(issue.milestone?.title ? { milestone: issue.milestone.title } : {}),
    comment_count: issue.comments ?? 0,
    is_pull_request: Boolean(issue.pull_request),
    html_url: issue.html_url,
    ...(issue.created_at ? { created_at: issue.created_at } : {}),
    ...(issue.updated_at ? { updated_at: issue.updated_at } : {}),
    ...(issue.closed_at ? { closed_at: issue.closed_at } : {}),
    ...(issue.body ? { body: issue.body } : {}),
  };
}

/** The people and teams a review has been requested from, as one list; a team is `team:slug`. */
export function reviewersOf(pull: GitHubObject | null | undefined): string[] {
  return [
    ...namesOf(pull?.requested_reviewers),
    ...(Array.isArray(pull?.requested_teams) ? pull.requested_teams : []).map((team: GitHubObject) => `team:${team.slug ?? team.name}`),
  ];
}

/** What the workflow tools report about a job of a run. */
export const jobShape = {
  id: z.number(),
  name: z.string(),
  status: z.string(),
  conclusion: z.string().optional(),
  failed_step: z.string().optional(),
};

/**
 * @param job A job as GitHub returns it.
 * @returns `failed_step` is the first step that failed, which
 *   is where to look in the job's log.
 */
export function jobFields(job: GitHubObject): z.infer<z.ZodObject<typeof jobShape>> {
  const steps = Array.isArray(job?.steps) ? job.steps : [];
  const failed = steps.find((step: GitHubObject | null) => step?.conclusion === "failure" || step?.conclusion === "timed_out");
  return {
    id: job.id,
    name: job.name ?? "",
    status: job.status ?? "unknown",
    ...(job.conclusion ? { conclusion: job.conclusion } : {}),
    ...(failed?.name ? { failed_step: failed.name } : {}),
  };
}
