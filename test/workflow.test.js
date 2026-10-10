import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as list, handle as workflowList } from "../src/tools/workflow-list.js";
import { TOOL as logs, handle as workflowLogs } from "../src/tools/workflow-logs.js";
import { TOOL as start, handle as workflowRun } from "../src/tools/workflow-run.js";
import { TOOL as status, handle as workflowStatus } from "../src/tools/workflow-status.js";
import { expectOutput, isRefusal, mockApi, reply, withToken } from "./helpers.js";

const SHA = "0123456789abcdef0123456789abcdef01234567";
const BASE = "https://api.github.com/repos/o/r";

/** A workflow as GitHub returns it. */
const workflow = (overrides = {}) => ({
  id: 161335,
  name: "CI",
  path: ".github/workflows/ci.yml",
  state: "active",
  ...overrides,
});

/** A workflow run as GitHub returns it. */
const run = (overrides = {}) => ({
  id: 77,
  name: "CI",
  display_title: "Add login",
  head_branch: "main",
  head_sha: SHA,
  event: "push",
  status: "completed",
  conclusion: "failure",
  run_number: 12,
  run_attempt: 1,
  actor: { login: "ada" },
  created_at: "2026-10-10T10:00:00Z",
  updated_at: "2026-10-10T10:05:00Z",
  html_url: "https://github.com/o/r/actions/runs/77",
  ...overrides,
});

/** A job as GitHub returns it. */
const job = (overrides = {}) => ({
  id: 1,
  name: "build",
  status: "completed",
  conclusion: "success",
  steps: [{ name: "Set up job", status: "completed", conclusion: "success", number: 1 }],
  ...overrides,
});

const failedJob = (id, name = `job-${id}`) =>
  job({
    id,
    name,
    conclusion: "failure",
    steps: [
      { name: "Set up job", status: "completed", conclusion: "success", number: 1 },
      { name: "Run tests", status: "completed", conclusion: "failure", number: 2 },
      { name: "Post job cleanup", status: "completed", conclusion: "success", number: 3 },
    ],
  });

/* ------------------------------------------------------------------ workflow_list */

test("workflow_list reports each workflow and whether another page follows", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(
      200,
      {
        total_count: 3,
        workflows: [
          workflow(),
          workflow({ id: 2, name: "Release", path: ".github/workflows/release.yml", state: "disabled_manually" }),
        ],
      },
      { link: '<https://api.github.com/repositories/1/actions/workflows?page=2>; rel="next"' },
    ),
  );

  const { payload } = await workflowList({ repo: "o/r", per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(calls[0].method, "GET");
  assert.equal(url.pathname, "/repos/o/r/actions/workflows");
  assert.deepEqual(Object.fromEntries(url.searchParams), { page: "1", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 of 3 workflows of o/r, on page 1.",
    items: [
      { id: 161335, name: "CI", path: ".github/workflows/ci.yml", state: "active" },
      { id: 2, name: "Release", path: ".github/workflows/release.yml", state: "disabled_manually" },
    ],
    total_count: 3,
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("workflow_list says so when there are none, and uses the singular for one", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { total_count: 0, workflows: [] }), reply(200, { total_count: 1, workflows: [workflow()] }));

  const none = await workflowList({ repo: "o/r" });
  assert.equal(none.payload.summary, "o/r has no workflows on page 1.");
  assert.equal(none.payload.has_more, false);
  expectOutput(list, none.payload);

  const one = await workflowList({ repo: "o/r" });
  assert.equal(one.payload.summary, "1 of 1 workflow of o/r, on page 1.");
});

test("workflow_list refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t);

  await assert.rejects(workflowList({ repo: "nope" }), isRefusal(/is not a repository/));
  await assert.rejects(workflowList({ repo: "o/r", per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100/));
  await assert.rejects(workflowList({ repo: "o/r", page: 0 }), isRefusal(/`page` must be between 1/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ workflow_run */

test("workflow_run starts the workflow on the default branch and reports the run GitHub names", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { default_branch: "main" }),
    reply(200, {
      workflow_run_id: 42,
      run_url: "https://api.github.com/repos/o/r/actions/runs/42",
      html_url: "https://github.com/o/r/actions/runs/42",
    }),
  );

  const { payload } = await workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: { level: "debug", retries: 3, dry: true } });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, BASE);
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, `${BASE}/actions/workflows/ci.yml/dispatches`);
  assert.deepEqual(calls[1].body, {
    ref: "main",
    inputs: { level: "debug", retries: "3", dry: "true" },
    return_run_details: true,
  });
  assert.deepEqual(payload, {
    summary: "Started workflow 'ci.yml' on main in o/r: run 42.",
    dispatched: true,
    workflow: "ci.yml",
    ref: "main",
    run_id: 42,
    html_url: "https://github.com/o/r/actions/runs/42",
  });
  expectOutput(start, payload);
});

test("workflow_run works when GitHub starts the run but does not say which", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204), reply(200, {}));

  const empty = await workflowRun({ repo: "o/r", workflow: "161335", ref: "release/1.0" });
  assert.equal(calls.length, 1, "a ref that is given needs no lookup of the default branch");
  assert.equal(calls[0].url, `${BASE}/actions/workflows/161335/dispatches`);
  assert.deepEqual(calls[0].body, { ref: "release/1.0", return_run_details: true });
  assert.deepEqual(empty.payload, {
    summary:
      "Asked GitHub to run workflow '161335' on release/1.0 in o/r. GitHub did not say which run it started; find it with workflow_status in a few seconds.",
    dispatched: true,
    workflow: "161335",
    ref: "release/1.0",
  });
  expectOutput(start, empty.payload);

  const bare = await workflowRun({ repo: "o/r", workflow: "ci.yml", ref: "main" });
  assert.equal(bare.payload.run_id, undefined);
  assert.equal(bare.payload.dispatched, true);
});

test("workflow_run takes the workflow as a file name, a path under .github/workflows, or an id", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204), reply(204), reply(204));

  await workflowRun({ repo: "o/r", workflow: ".github/workflows/release.yaml", ref: "main" });
  await workflowRun({ repo: "o/r", workflow: " 161335 ", ref: "main" });
  await workflowRun({ repo: "o/r", workflow: "build_and-test.v2.yml", ref: "v1.2.3" });

  assert.deepEqual(
    calls.map((call) => call.url),
    [
      `${BASE}/actions/workflows/release.yaml/dispatches`,
      `${BASE}/actions/workflows/161335/dispatches`,
      `${BASE}/actions/workflows/build_and-test.v2.yml/dispatches`,
    ],
  );
});

test("workflow_run sends no inputs when there are none to send", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204), reply(204));

  await workflowRun({ repo: "o/r", workflow: "ci.yml", ref: "main", inputs: {} });
  await workflowRun({ repo: "o/r", workflow: "ci.yml", ref: "main", inputs: null });

  assert.deepEqual(calls[0].body, { ref: "main", return_run_details: true });
  assert.deepEqual(calls[1].body, { ref: "main", return_run_details: true });
});

test("workflow_run refuses bad arguments before any request, and a repository whose default branch it cannot find", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(workflowRun({ repo: "o/r" }), isRefusal(/`workflow` is required/));
  await assert.rejects(workflowRun({ repo: "o/r", workflow: "a/b.yml" }), isRefusal(/is not a workflow/));
  await assert.rejects(workflowRun({ repo: "o/r", workflow: "ci.yml", ref: "a b" }), isRefusal(/`ref` 'a b' is not a valid/));
  await assert.rejects(workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: "x=1" }), isRefusal(/`inputs` must be an object/));
  await assert.rejects(workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: ["x"] }), isRefusal(/`inputs` must be an object/));
  await assert.rejects(
    workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: { a: { nested: 1 } } }),
    isRefusal(/`inputs\.a` must be a string, a number or true\/false/),
  );
  await assert.rejects(
    workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: { a: null } }),
    isRefusal(/`inputs\.a` must be a string, a number or true\/false/),
  );
  await assert.rejects(workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: { " ": "x" } }), isRefusal(/blank name/));
  const many = Object.fromEntries(Array.from({ length: 26 }, (_, i) => [`k${i}`, "v"]));
  await assert.rejects(workflowRun({ repo: "o/r", workflow: "ci.yml", inputs: many }), isRefusal(/at most 25 inputs, got 26/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    workflowRun({ repo: "o/r", workflow: "ci.yml" }),
    isRefusal(/Could not find the default branch of o\/r; pass `ref`/),
  );
});

test("workflow_run explains a workflow it cannot find and one that cannot be dispatched", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(404, { message: "Not Found" }),
    reply(422, { message: "Workflow does not have 'workflow_dispatch' trigger" }),
    reply(403, { message: "Resource not accessible by personal access token" }, { "x-accepted-github-permissions": "actions=write" }),
  );

  await assert.rejects(
    workflowRun({ repo: "o/r", workflow: "nope.yml", ref: "main" }),
    isRefusal(/could not find the workflow 'nope\.yml' in o\/r \(404\).*Check it with workflow_list.*default branch/),
  );
  await assert.rejects(
    workflowRun({ repo: "o/r", workflow: "ci.yml", ref: "main" }),
    isRefusal(/would not start 'ci\.yml' on 'main' in o\/r \(422\).*does not have 'workflow_dispatch' trigger.*on: workflow_dispatch/),
  );
  await assert.rejects(
    workflowRun({ repo: "o/r", workflow: "ci.yml", ref: "main" }),
    isRefusal(/refused to run the workflow 'ci\.yml' of o\/r on 'main' \(403\).*actions=write/),
  );
});

/* ------------------------------------------------------------------ workflow_status */

test("workflow_status reads a run by id with its jobs and where the failed one failed", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, run()),
    reply(200, { total_count: 2, jobs: [job(), failedJob(2, "test")] }),
  );

  const { payload } = await workflowStatus({ repo: "o/r", run_id: 77 });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, `${BASE}/actions/runs/77`);
  assert.equal(calls[1].url, `${BASE}/actions/runs/77/jobs?per_page=100`);
  assert.deepEqual(payload, {
    summary: `Workflow run 77 (CI) on main at 0123456 in o/r is completed, failure. Failed: test.`,
    found: true,
    run_id: 77,
    name: "CI",
    status: "completed",
    conclusion: "failure",
    branch: "main",
    sha: SHA,
    event: "push",
    run_number: 12,
    attempt: 1,
    actor: "ada",
    created_at: "2026-10-10T10:00:00Z",
    updated_at: "2026-10-10T10:05:00Z",
    html_url: "https://github.com/o/r/actions/runs/77",
    job_count: 2,
    jobs: [
      { id: 1, name: "build", status: "completed", conclusion: "success" },
      { id: 2, name: "test", status: "completed", conclusion: "failure", failed_step: "Run tests" },
    ],
  });
  expectOutput(status, payload);
});

test("workflow_status finds the latest run of a workflow, on a branch, and has no conclusion while it runs", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { total_count: 5, workflow_runs: [run({ status: "in_progress", conclusion: null })] }),
    reply(200, { total_count: 1, jobs: [job({ status: "in_progress", conclusion: null })] }),
  );

  const { payload } = await workflowStatus({ repo: "o/r", workflow: "ci.yml", branch: "release/1.0" });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/repos/o/r/actions/workflows/ci.yml/runs");
  assert.deepEqual(Object.fromEntries(url.searchParams), { per_page: "1", branch: "release/1.0" });
  assert.equal(calls[1].url, `${BASE}/actions/runs/77/jobs?per_page=100`);
  assert.equal(payload.summary, "Workflow run 77 (CI) on main at 0123456 in o/r is in_progress.");
  assert.equal("conclusion" in payload, false);
  assert.deepEqual(payload.jobs, [{ id: 1, name: "build", status: "in_progress" }]);
  expectOutput(status, payload);
});

test("workflow_status without a branch asks for the workflow's runs on any branch", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { workflow_runs: [run({ conclusion: "success" })] }), reply(200, { total_count: 0, jobs: [] }));

  const { payload } = await workflowStatus({ repo: "o/r", workflow: "161335" });

  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { per_page: "1" });
  assert.equal(new URL(calls[0].url).pathname, "/repos/o/r/actions/workflows/161335/runs");
  assert.equal(payload.summary, "Workflow run 77 (CI) on main at 0123456 in o/r is completed, success.");
  assert.equal(payload.job_count, 0);
  assert.deepEqual(payload.jobs, []);
  expectOutput(status, payload);
});

test("workflow_status says there is no run yet, as an answer and not an error", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { total_count: 0, workflow_runs: [] }), reply(200, { total_count: 0, workflow_runs: [] }));

  const any = await workflowStatus({ repo: "o/r", workflow: "ci.yml" });
  assert.deepEqual(any.payload, {
    summary:
      "No run of workflow 'ci.yml' in o/r yet. A run appears a few seconds after it is triggered; ask again shortly.",
    found: false,
    jobs: [],
  });
  assert.equal(any.ok, true);
  expectOutput(status, any.payload);

  const onBranch = await workflowStatus({ repo: "o/r", workflow: "ci.yml", branch: "dev" });
  assert.match(onBranch.payload.summary, /^No run of workflow 'ci\.yml' on dev in o\/r yet\./);
  assert.equal(calls.length, 2, "no jobs are asked for when there is no run");
});

test("workflow_status refuses an unclear question before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t);

  await assert.rejects(workflowStatus({ repo: "o/r" }), isRefusal(/Give `run_id`, or `workflow` to read its latest run/));
  await assert.rejects(workflowStatus({ repo: "o/r", run_id: 1, workflow: "ci.yml" }), isRefusal(/either `run_id` or `workflow`, not both/));
  await assert.rejects(workflowStatus({ repo: "o/r", run_id: 1, branch: "main" }), isRefusal(/`branch` only narrows the search/));
  await assert.rejects(workflowStatus({ repo: "o/r", run_id: 0 }), isRefusal(/`run_id` must be between 1/));
  await assert.rejects(workflowStatus({ repo: "o/r", workflow: "a/b" }), isRefusal(/is not a workflow/));
  await assert.rejects(workflowStatus({ repo: "o/r", workflow: "ci.yml", branch: "a b" }), isRefusal(/`branch` 'a b' is not a valid/));
  assert.equal(calls.length, 0);
});

test("workflow_status says which run or workflow it could not find", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }), reply(404, { message: "Not Found" }));

  await assert.rejects(
    workflowStatus({ repo: "o/r", run_id: 9 }),
    isRefusal(/could not find workflow run 9 in o\/r \(404\).*Give a workflow to read its latest run/),
  );
  await assert.rejects(
    workflowStatus({ repo: "o/r", workflow: "nope.yml" }),
    isRefusal(/could not find the workflow 'nope\.yml' in o\/r \(404\).*workflow_list/),
  );
});

test("workflow_status refuses a body that is not a run", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { message: "hello" }));

  await assert.rejects(workflowStatus({ repo: "o/r", run_id: 77 }), isRefusal(/not a workflow run this server recognizes/));
});

/* ------------------------------------------------------------------ workflow_logs */

const RAW_LOG =
  "﻿2026-10-10T10:00:00.0000000Z ##[group]Run npm test\r\n" +
  "2026-10-10T10:00:01.0000000Z \u001b[31;1mError: boom\u001b[0m\r\n" +
  "2026-10-10T10:00:02.0000000Z ##[error]Process completed with exit code 1.\r\n";

test("workflow_logs reads the failed jobs' logs by default, cleaned of what only a terminal wants", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { total_count: 2, jobs: [job(), failedJob(2, "test")] }),
    reply(200, RAW_LOG),
  );

  const { payload } = await workflowLogs({ repo: "o/r", run_id: 77 });

  assert.equal(calls.length, 2, "the passing job's log is not fetched");
  assert.equal(calls[0].url, `${BASE}/actions/runs/77/jobs?per_page=100`);
  assert.equal(calls[1].url, `${BASE}/actions/jobs/2/logs`);
  assert.equal(calls[1].headers.Accept, "application/vnd.github+json");
  assert.deepEqual(payload, {
    summary: "Logs of 1 failed job in run 77 of o/r.",
    run_id: 77,
    job_count: 1,
    jobs: [
      {
        id: 2,
        name: "test",
        status: "completed",
        conclusion: "failure",
        failed_step: "Run tests",
        log:
          "2026-10-10T10:00:00.0000000Z ##[group]Run npm test\n" +
          "2026-10-10T10:00:01.0000000Z Error: boom\n" +
          "2026-10-10T10:00:02.0000000Z ##[error]Process completed with exit code 1.",
        truncated: false,
      },
    ],
    truncated: false,
  });
  expectOutput(logs, payload);
});

test("workflow_logs shares its budget between jobs and keeps the end of a long log, where the failure is", async (t) => {
  withToken(t);
  const long = (label) => `${"x".repeat(30_000)}\n##[error]${label} failed`;
  const { calls } = mockApi(
    t,
    reply(200, { total_count: 2, jobs: [failedJob(2, "unit"), failedJob(3, "e2e")] }),
    reply(200, long("unit")),
    reply(200, long("e2e")),
  );

  const { payload } = await workflowLogs({ repo: "o/r", run_id: 77 });

  assert.equal(calls.length, 3);
  assert.equal(payload.summary, "Logs of 2 failed jobs in run 77 of o/r. A long log is cut to its last 10000 characters.");
  assert.equal(payload.truncated, true);
  for (const [index, label] of ["unit", "e2e"].entries()) {
    const { log, truncated } = payload.jobs[index];
    assert.equal(truncated, true);
    assert.ok(log.startsWith("(truncated) ...\n"));
    assert.ok(log.endsWith(`##[error]${label} failed`));
    assert.equal(log.length, "(truncated) ...\n".length + 10_000);
  }
  expectOutput(logs, payload);
});

test("workflow_logs reads at most five jobs at once and says how many it left", async (t) => {
  withToken(t);
  const ids = [1, 2, 3, 4, 5, 6, 7];
  const { calls } = mockApi(
    t,
    reply(200, { total_count: 7, jobs: ids.map((id) => failedJob(id)) }),
    ...ids.slice(0, 5).map((id) => reply(200, `log of ${id}`)),
  );

  const { payload } = await workflowLogs({ repo: "o/r", run_id: 77 });

  assert.equal(calls.length, 6);
  assert.deepEqual(payload.jobs.map((entry) => entry.id), [1, 2, 3, 4, 5]);
  assert.equal(payload.job_count, 7);
  assert.equal(payload.truncated, true);
  assert.equal(
    payload.summary,
    "Logs of 5 failed jobs in run 77 of o/r. Showing the first 5 of 7; pass job_id for the others.",
  );
  expectOutput(logs, payload);
});

test("workflow_logs reads one job when asked, whatever its result", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { total_count: 2, jobs: [job(), failedJob(2, "test")] }), reply(200, "all fine"));

  const { payload } = await workflowLogs({ repo: "o/r", run_id: 77, job_id: 1, only_failed: true });

  assert.equal(calls.length, 2);
  assert.equal(calls[1].url, `${BASE}/actions/jobs/1/logs`);
  assert.equal(payload.summary, "Log of job 'build' (1) in run 77 of o/r.");
  assert.equal(payload.jobs[0].log, "all fine");
  assert.equal(payload.job_count, 1);
  expectOutput(logs, payload);
});

test("workflow_logs refuses a job that is not in the run", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { total_count: 1, jobs: [job()] }));

  await assert.rejects(
    workflowLogs({ repo: "o/r", run_id: 77, job_id: 99 }),
    isRefusal(/Job 99 is not among the 1 jobs listed for run 77 in o\/r\. Read the run with workflow_status/),
  );
});

test("workflow_logs reads every job when only_failed is false", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { total_count: 2, jobs: [job(), failedJob(2, "test")] }), reply(200, "one"), reply(200, "two"));

  const { payload } = await workflowLogs({ repo: "o/r", run_id: 77, only_failed: false });

  assert.equal(calls.length, 3);
  assert.equal(payload.summary, "Logs of 2 jobs in run 77 of o/r.");
  assert.deepEqual(payload.jobs.map((entry) => entry.log), ["one", "two"]);
  expectOutput(logs, payload);
});

test("workflow_logs treats a missing or expired log as a note on that job, and carries on with the others", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, { total_count: 3, jobs: [failedJob(1, "a"), failedJob(2, "b"), failedJob(3, "c")] }),
    reply(404, { message: "Not Found" }),
    reply(410, { message: "Gone" }),
    reply(200, "   \r\n"),
  );

  const { payload } = await workflowLogs({ repo: "o/r", run_id: 77 });

  const note = "GitHub has no log for this job: it may not have started, or the log has expired.";
  assert.equal(payload.jobs[0].note, note);
  assert.equal(payload.jobs[1].note, note);
  assert.equal(payload.jobs[2].note, "The log is empty.");
  assert.ok(payload.jobs.every((entry) => entry.log === undefined && entry.truncated === false));
  assert.equal(payload.truncated, false);
  expectOutput(logs, payload);
});

test("workflow_logs says why there is nothing to read", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, { total_count: 2, jobs: [job(), job({ id: 2, name: "test" })] }),
    reply(200, { total_count: 2, jobs: [job(), job({ id: 2, name: "test", status: "in_progress", conclusion: null })] }),
    reply(200, { total_count: 0, jobs: [] }),
  );

  const passed = await workflowLogs({ repo: "o/r", run_id: 77 });
  assert.deepEqual(passed.payload, {
    summary: "Run 77 in o/r has no failed jobs. Pass only_failed false to read every job, or job_id for one.",
    run_id: 77,
    job_count: 0,
    jobs: [],
    truncated: false,
  });
  expectOutput(logs, passed.payload);

  const running = await workflowLogs({ repo: "o/r", run_id: 77 });
  assert.equal(
    running.payload.summary,
    "Run 77 in o/r has no failed jobs so far, and some jobs have not finished. Pass only_failed false to read every job, or job_id for one.",
  );

  const queued = await workflowLogs({ repo: "o/r", run_id: 77 });
  assert.equal(queued.payload.summary, "Run 77 in o/r has no jobs yet.");
});

test("workflow_logs does not swallow a refusal that is not about the log", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { total_count: 1, jobs: [failedJob(1)] }), reply(403, { message: "Resource not accessible by personal access token" }));

  await assert.rejects(workflowLogs({ repo: "o/r", run_id: 77 }), isRefusal(/refused to read the log of job 1 of o\/r \(403\)/));
});

test("workflow_logs says which run it could not find, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }));

  await assert.rejects(workflowLogs({ repo: "o/r" }), isRefusal(/`run_id` is required/));
  await assert.rejects(workflowLogs({ repo: "o/r", run_id: 1.5 }), isRefusal(/`run_id` must be a whole number/));
  await assert.rejects(workflowLogs({ repo: "o/r", run_id: 1, job_id: 0 }), isRefusal(/`job_id` must be between 1/));
  await assert.rejects(workflowLogs({ repo: "o/r", run_id: 1, only_failed: "no" }), isRefusal(/`only_failed` must be true or false/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    workflowLogs({ repo: "o/r", run_id: 9 }),
    isRefusal(/could not find workflow run 9 in o\/r \(404\).*Find run ids with workflow_status/),
  );
});
