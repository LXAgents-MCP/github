/** Read the state of a GitHub Actions workflow run and its jobs. */
import { z } from "zod";
import { github, parseRepo, refName, workflowName } from "../api.js";
import { optionalInt, optionalText } from "../args.js";
import { MAX_PER_PAGE } from "../config.js";
import { Refused } from "../errors.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody, jobFields, jobShape } from "../shapes.js";
import { short } from "../text.js";

export const TOOL = {
  name: "workflow_status",
  description:
    "Read a GitHub Actions workflow run: its status and conclusion, branch, commit, and each job's result with " +
    "the step that failed. Give a run_id, or a workflow (optionally on a branch) to read its latest run. After " +
    "workflow_run, ask again until the status is 'completed'.",
  inputSchema: {
    repo: repoArg,
    run_id: z.number().int().optional().describe("Run id, from workflow_run or an earlier workflow_status."),
    workflow: z.string().optional().describe("Workflow id or file name, e.g. 'ci.yml'; reads its most recent run."),
    branch: z.string().optional().describe("With `workflow`: only runs on this branch."),
  },
  outputSchema: {
    found: z.boolean(),
    run_id: z.number().optional(),
    name: z.string().optional(),
    status: z.string().optional(),
    conclusion: z.string().optional(),
    branch: z.string().optional(),
    sha: z.string().optional(),
    event: z.string().optional(),
    run_number: z.number().optional(),
    attempt: z.number().optional(),
    actor: z.string().optional(),
    created_at: z.string().optional(),
    updated_at: z.string().optional(),
    html_url: z.string().optional(),
    job_count: z.number().optional(),
    jobs: z.array(z.object(jobShape)),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, run_id?: unknown, workflow?: unknown, branch?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const runId = optionalInt(args.run_id, "run_id", { min: 1 });
  const workflow = optionalText(args.workflow, "workflow") === undefined ? undefined : workflowName(args.workflow);
  const branch = optionalText(args.branch, "branch") === undefined ? undefined : refName(args.branch, "branch");

  if (runId !== undefined && workflow !== undefined) throw new Refused("Give either `run_id` or `workflow`, not both.");
  if (runId === undefined && workflow === undefined) {
    throw new Refused("Give `run_id`, or `workflow` to read its latest run.");
  }
  if (branch !== undefined && workflow === undefined) {
    throw new Refused("`branch` only narrows the search for a workflow's latest run; give `workflow` with it.");
  }

  let run;
  if (runId !== undefined) {
    run = (
      await github("GET", `${repo.path}/actions/runs/${runId}`, {
        what: `read workflow run ${runId} of ${repo.full}`,
        explain: (status, detail) =>
          status === 404
            ? `GitHub could not find workflow run ${runId} in ${repo.full} (404)${detail}. Give a workflow to read its ` +
              "latest run and find the id. GitHub also answers 404 for a private repository the token cannot see."
            : undefined,
      })
    ).body;
    expectBody(run, "html_url", "a workflow run");
  } else {
    const { body } = await github("GET", `${repo.path}/actions/workflows/${workflow.encoded}/runs`, {
      query: { per_page: 1, branch },
      what: `find the latest run of the workflow '${workflow.name}' of ${repo.full}`,
      explain: (status, detail) =>
        status === 404
          ? `GitHub could not find the workflow '${workflow.name}' in ${repo.full} (404)${detail}. Check it with ` +
            "workflow_list. GitHub also answers 404 for a private repository the token cannot see."
          : undefined,
    });
    run = Array.isArray(body?.workflow_runs) ? body.workflow_runs[0] : undefined;
    if (run === undefined) {
      return {
        payload: {
          summary:
            `No run of workflow '${workflow.name}'${branch === undefined ? "" : ` on ${branch}`} in ${repo.full} yet. ` +
            "A run appears a few seconds after it is triggered; ask again shortly.",
          found: false,
          jobs: [],
        },
        ok: true,
      };
    }
    expectBody(run, "html_url", "a workflow run");
  }

  const { body: listing } = await github("GET", `${repo.path}/actions/runs/${run.id}/jobs`, {
    query: { per_page: MAX_PER_PAGE },
    what: `list the jobs of workflow run ${run.id} of ${repo.full}`,
  });
  const jobs = (Array.isArray(listing?.jobs) ? listing.jobs : []).map(jobFields);
  const failed = jobs.filter((job) => job.conclusion === "failure" || job.conclusion === "timed_out");

  const name = run.name ?? run.display_title ?? "";
  const where = run.head_branch ? ` on ${run.head_branch}` : "";
  const state = run.status === "completed" ? `completed, ${run.conclusion ?? "with no conclusion"}` : run.status;
  return {
    payload: {
      summary:
        `Workflow run ${run.id}${name ? ` (${name})` : ""}${where} at ${short(run.head_sha)} in ${repo.full} is ${state}.` +
        (failed.length > 0 ? ` Failed: ${failed.map((job) => job.name).join(", ")}.` : ""),
      found: true,
      run_id: run.id,
      ...(name ? { name } : {}),
      ...(run.status ? { status: run.status } : {}),
      ...(run.conclusion ? { conclusion: run.conclusion } : {}),
      ...(run.head_branch ? { branch: run.head_branch } : {}),
      ...(run.head_sha ? { sha: run.head_sha } : {}),
      ...(run.event ? { event: run.event } : {}),
      ...(Number.isInteger(run.run_number) ? { run_number: run.run_number } : {}),
      ...(Number.isInteger(run.run_attempt) ? { attempt: run.run_attempt } : {}),
      ...(run.actor?.login ? { actor: run.actor.login } : {}),
      ...(run.created_at ? { created_at: run.created_at } : {}),
      ...(run.updated_at ? { updated_at: run.updated_at } : {}),
      html_url: run.html_url,
      job_count: Number.isInteger(listing?.total_count) ? listing.total_count : jobs.length,
      jobs,
    },
    ok: true,
  };
}
