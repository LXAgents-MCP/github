/** Read the logs of the jobs of a GitHub Actions workflow run. */
import { z } from "zod";
import { github, parseRepo } from "../services/api.js";
import { optionalFlag, optionalInt, requiredInt } from "../models/args.js";
import { MAX_LOG_CHARS, MAX_LOG_JOBS, MAX_PER_PAGE } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { READ, repoArg } from "../models/schemas.js";
import { jobFields, jobShape } from "../models/shapes.js";
import { cleanLog, tail } from "../utils/text.js";

const isFailed = (job) => job.conclusion === "failure" || job.conclusion === "timed_out";

export const TOOL = {
  name: "workflow_logs",
  description:
    "Read the logs of the jobs of a GitHub Actions workflow run. By default only the failed jobs, up to 5, each " +
    "cut to its last lines, where a failure is explained; job_id reads one job and only_failed false reads every " +
    "job. At most 20,000 characters in all. Find run and job ids with workflow_status.",
  inputSchema: {
    repo: repoArg,
    run_id: z.number().int().describe("Workflow run id."),
    job_id: z.number().int().optional().describe("Read just this job of the run. Default: the failed jobs."),
    only_failed: z
      .boolean()
      .optional()
      .describe("false reads every job of the run, not only the failed ones. Default true. Ignored with job_id."),
  },
  outputSchema: {
    run_id: z.number(),
    job_count: z.number(),
    jobs: z.array(
      z.object({ ...jobShape, log: z.string().optional(), truncated: z.boolean(), note: z.string().optional() }),
    ),
    truncated: z.boolean(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, run_id?: unknown, job_id?: unknown, only_failed?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const runId = requiredInt(args.run_id, "run_id", { min: 1 });
  const jobId = optionalInt(args.job_id, "job_id", { min: 1 });
  const onlyFailed = optionalFlag(args.only_failed, "only_failed", true);

  const { body: listing } = await github("GET", `${repo.path}/actions/runs/${runId}/jobs`, {
    query: { per_page: MAX_PER_PAGE },
    what: `list the jobs of workflow run ${runId} of ${repo.full}`,
    explain: (status, detail) =>
      status === 404
        ? `GitHub could not find workflow run ${runId} in ${repo.full} (404)${detail}. Find run ids with ` +
          "workflow_status. GitHub also answers 404 for a private repository the token cannot see."
        : undefined,
  });
  const all = Array.isArray(listing?.jobs) ? listing.jobs : [];

  let selected;
  if (jobId !== undefined) {
    selected = all.filter((job) => job.id === jobId);
    if (selected.length === 0) {
      throw new Refused(
        `Job ${jobId} is not among the ${all.length} jobs listed for run ${runId} in ${repo.full}. ` +
          "Read the run with workflow_status to see its jobs.",
      );
    }
  } else {
    selected = onlyFailed ? all.filter(isFailed) : all;
  }

  if (selected.length === 0) {
    const unfinished = all.some((job) => job.status !== "completed");
    return {
      payload: {
        summary:
          all.length === 0
            ? `Run ${runId} in ${repo.full} has no jobs yet.`
            : `Run ${runId} in ${repo.full} has no failed jobs${unfinished ? " so far, and some jobs have not finished" : ""}. ` +
              "Pass only_failed false to read every job, or job_id for one.",
        run_id: runId,
        job_count: 0,
        jobs: [],
        truncated: false,
      },
      ok: true,
    };
  }

  // The jobs share the budget, one request each and one after another, so a matrix of failures
  // is not a burst against the rate limit.
  const reading = selected.slice(0, MAX_LOG_JOBS);
  const share = Math.floor(MAX_LOG_CHARS / reading.length);
  const jobs = [];
  let cut = selected.length > reading.length;

  for (const job of reading) {
    const fields = jobFields(job);
    const answer = await github("GET", `${repo.path}/actions/jobs/${job.id}/logs`, {
      raw: true,
      allow: [404, 410],
      what: `read the log of job ${job.id} of ${repo.full}`,
    });

    if (answer.status === 404 || answer.status === 410) {
      jobs.push({ ...fields, truncated: false, note: "GitHub has no log for this job: it may not have started, or the log has expired." });
      continue;
    }
    const text = cleanLog(typeof answer.body === "string" ? answer.body : "").trimEnd();
    if (text === "") {
      jobs.push({ ...fields, truncated: false, note: "The log is empty." });
      continue;
    }
    const [log, shortened] = tail(text, share);
    cut = cut || shortened;
    jobs.push({ ...fields, log, truncated: shortened });
  }

  const summary =
    jobId !== undefined
      ? `Log of job '${jobs[0].name}' (${jobId}) in run ${runId} of ${repo.full}.`
      : `Logs of ${jobs.length} ${onlyFailed ? "failed " : ""}${jobs.length === 1 ? "job" : "jobs"} in run ${runId} of ${repo.full}.` +
        (selected.length > reading.length
          ? ` Showing the first ${reading.length} of ${selected.length}; pass job_id for the others.`
          : "");
  return {
    payload: {
      summary: jobs.some((job) => job.truncated) ? `${summary} A long log is cut to its last ${share} characters.` : summary,
      run_id: runId,
      job_count: selected.length,
      jobs,
      truncated: cut,
    },
    ok: true,
  };
}
