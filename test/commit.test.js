import assert from "node:assert/strict";
import test from "node:test";
import { MAX_COMMITS_SHOWN, MAX_DIFF_CHARS, MAX_FILES_SHOWN } from "../src/config/config.js";
import { TOOL as compare, handle as commitCompare } from "../src/controllers/commit-compare.js";
import { TOOL as list, handle as commitList } from "../src/controllers/commit-list.js";
import { TOOL as read, handle as commitRead } from "../src/controllers/commit-read.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const SHA = "0123456789abcdef0123456789abcdef01234567";
const PARENT = "fedcba9876543210fedcba9876543210fedcba98";

const commitBody = (overrides = {}) => ({
  sha: SHA,
  html_url: `https://github.com/o/r/commit/${SHA}`,
  commit: {
    message: "Add login\n\nThe long explanation\nover two lines.",
    author: { name: "Ada", date: "2026-10-01T12:00:00Z" },
  },
  parents: [{ sha: PARENT }],
  stats: { additions: 12, deletions: 3, total: 15 },
  files: [
    { filename: "src/a.js", status: "added", additions: 10, deletions: 0, patch: "@@" },
    { filename: "src/b.js", status: "modified", additions: 2, deletions: 3 },
  ],
  ...overrides,
});

const manyFiles = (n) => Array.from({ length: n }, (_, i) => ({ filename: `f${i}.js`, status: "modified", additions: 1, deletions: 0 }));

/* ------------------------------------------------------------------ commit_read */

test("commit_read reports the message, author, parents and changed files", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, commitBody()));

  const { payload } = await commitRead({ repo: "o/r", ref: "feat/login" });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/commits/feat/login");
  assert.deepEqual(payload, {
    summary: "Commit 0123456 by Ada: Add login",
    sha: SHA,
    message: "Add login\n\nThe long explanation\nover two lines.",
    author: "Ada",
    date: "2026-10-01T12:00:00Z",
    parents: [PARENT],
    additions: 12,
    deletions: 3,
    file_count: 2,
    files: [
      { filename: "src/a.js", status: "added", additions: 10, deletions: 0 },
      { filename: "src/b.js", status: "modified", additions: 2, deletions: 3 },
    ],
    truncated: false,
    html_url: `https://github.com/o/r/commit/${SHA}`,
  });
  expectOutput(read, payload);
});

test("commit_read defaults to the tip of the default branch and copes with a bare commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { sha: SHA, commit: { message: "" }, parents: [] }));

  const { payload } = await commitRead({ repo: "o/r" });

  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/commits/HEAD");
  assert.equal(payload.summary, "Commit 0123456: (no message)");
  assert.deepEqual(payload.files, []);
  assert.equal(payload.additions, 0);
  assert.equal(payload.file_count, 0);
  expectOutput(read, payload);
});

test("commit_read asks for the diff with its own media type and cuts a long one", async (t) => {
  withToken(t);
  const diff = `diff --git a/x b/x\n${"+line\n".repeat(MAX_DIFF_CHARS)}`;
  const { calls } = mockApi(t, reply(200, commitBody()), reply(200, diff));

  const { payload } = await commitRead({ repo: "o/r", ref: SHA, diff: true });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].headers.Accept, "application/vnd.github+json");
  assert.equal(calls[1].headers.Accept, "application/vnd.github.diff");
  assert.equal(calls[1].url, calls[0].url);
  assert.equal(payload.truncated, true);
  assert.ok(payload.diff.startsWith("diff --git a/x b/x"));
  assert.ok(payload.diff.length <= MAX_DIFF_CHARS + 20);
  expectOutput(read, payload);
});

test("commit_read shows the first hundred files and reports how many there are", async (t) => {
  withToken(t);
  mockApi(t, reply(200, commitBody({ files: manyFiles(MAX_FILES_SHOWN + 20) })));

  const { payload } = await commitRead({ repo: "o/r" });

  assert.equal(payload.files.length, MAX_FILES_SHOWN);
  assert.equal(payload.file_count, MAX_FILES_SHOWN + 20);
  assert.equal(payload.truncated, true);
});

test("commit_read says a huge diff timed out, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, commitBody()), reply(504, { message: "Gateway Timeout" }));

  await assert.rejects(commitRead({ repo: "o/r", diff: true }), isRefusal(/could not produce the diff of 'HEAD' in o\/r \(504\)\. Very large diffs time out/));
  assert.equal(calls.length, 2);

  await assert.rejects(commitRead({ repo: "o/r", ref: "a b" }), isRefusal(/`ref` 'a b' is not a valid/));
  await assert.rejects(commitRead({ repo: "o/r", diff: "yes" }), isRefusal(/`diff` must be true or false/));
  assert.equal(calls.length, 2);
});

test("commit_read reports a commit that does not exist, and a body that is not a commit", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "No commit found for SHA: deadbeef" }), reply(200, { nope: 1 }));

  await assert.rejects(commitRead({ repo: "o/r", ref: "deadbeef" }), isRefusal(/could not find what it needs to read the commit 'deadbeef' of o\/r \(404\): No commit found/));
  await assert.rejects(commitRead({ repo: "o/r" }), isRefusal(/the body was not a commit this server recognizes/));
});

/* ------------------------------------------------------------------ commit_list */

test("commit_list sends the filters and the paging, and lists each commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(
      200,
      [
        { sha: SHA, commit: { message: "Newest\n\nbody", author: { name: "Ada", date: "2026-10-02T00:00:00Z" } } },
        { sha: PARENT, commit: { message: "Older", author: { name: "Bob", date: "2026-10-01T00:00:00Z" } } },
      ],
      { link: '<https://api.github.com/repos/o/r/commits?page=2>; rel="next"' },
    ),
  );

  const { payload } = await commitList({
    repo: "o/r",
    ref: "dev",
    path: "/src/",
    author: " ada ",
    since: "2026-10-01",
    until: "2026-10-31T23:59:59Z",
    per_page: 2,
  });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/repos/o/r/commits");
  assert.deepEqual(Object.fromEntries(url.searchParams), {
    sha: "dev",
    path: "src",
    author: "ada",
    since: "2026-10-01",
    until: "2026-10-31T23:59:59Z",
    page: "1",
    per_page: "2",
  });
  assert.deepEqual(payload, {
    summary: "2 commits on dev of o/r on page 1.",
    items: [
      { sha: SHA, message: "Newest", author: "Ada", date: "2026-10-02T00:00:00Z" },
      { sha: PARENT, message: "Older", author: "Bob", date: "2026-10-01T00:00:00Z" },
    ],
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("commit_list sends no filter by default and reports an empty page", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, []));

  const { payload } = await commitList({ repo: "o/r", ref: "dev", page: 9 });

  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { sha: "dev", page: "9", per_page: "30" });
  assert.equal(payload.summary, "No commits on dev of o/r match on page 9.");
  assert.deepEqual(payload.items, []);
  expectOutput(list, payload);
});

test("commit_list refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(commitList({ repo: "o/r", since: "yesterday" }), isRefusal(/`since` must be an ISO 8601 date/));
  await assert.rejects(commitList({ repo: "o/r", until: "soon" }), isRefusal(/`until` must be an ISO 8601 date/));
  await assert.rejects(commitList({ repo: "o/r", path: "../x" }), isRefusal(/is not a path inside the repository/));
  await assert.rejects(commitList({ repo: "o/r", ref: "a..b" }), isRefusal(/`ref` 'a\.\.b' is not a valid/));
  await assert.rejects(commitList({ repo: "o/r", author: 3 }), isRefusal(/`author` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ commit_compare */

const comparison = (overrides = {}) => ({
  status: "ahead",
  ahead_by: 2,
  behind_by: 0,
  total_commits: 2,
  html_url: "https://github.com/o/r/compare/main...feat",
  commits: [
    { sha: PARENT, commit: { message: "First\n\nbody", author: { name: "Ada" } } },
    { sha: SHA, commit: { message: "Second", author: { name: "Bob" } } },
  ],
  files: [{ filename: "a.js", status: "added", additions: 4, deletions: 0 }],
  ...overrides,
});

test("commit_compare reports how far ahead the head is, the commits and the files", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, comparison()));

  const { payload } = await commitCompare({ repo: "o/r", base: "main", head: "feat/login" });

  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/compare/main...feat/login");
  assert.deepEqual(payload, {
    summary: "feat/login is 2 commits ahead of main.",
    status: "ahead",
    ahead_by: 2,
    behind_by: 0,
    total_commits: 2,
    commits: [
      { sha: PARENT, message: "First", author: "Ada" },
      { sha: SHA, message: "Second", author: "Bob" },
    ],
    file_count: 1,
    files: [{ filename: "a.js", status: "added", additions: 4, deletions: 0 }],
    truncated: false,
    html_url: "https://github.com/o/r/compare/main...feat",
  });
  expectOutput(compare, payload);
});

test("commit_compare words each status", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, comparison({ status: "identical", ahead_by: 0, total_commits: 0, commits: [], files: [] })),
    reply(200, comparison({ status: "behind", ahead_by: 0, behind_by: 1 })),
    reply(200, comparison({ status: "diverged", ahead_by: 3, behind_by: 4 })),
    reply(200, comparison({ status: "ahead", ahead_by: 1 })),
    reply(200, comparison({ status: "mystery" })),
  );

  const said = async () => (await commitCompare({ repo: "o/r", base: "main", head: "x" })).payload.summary;
  assert.equal(await said(), "x and main are identical.");
  assert.equal(await said(), "x is 1 commit behind main.");
  assert.equal(await said(), "x has diverged from main: 3 ahead, 4 behind.");
  assert.equal(await said(), "x is 1 commit ahead of main.");
  assert.equal(await said(), "x compared with main: mystery.");
});

test("commit_compare keeps the newest commits and the first files when there are many", async (t) => {
  withToken(t);
  const commits = Array.from({ length: MAX_COMMITS_SHOWN + 10 }, (_, i) => ({ sha: `sha${i}`, commit: { message: `c${i}` } }));
  mockApi(t, reply(200, comparison({ commits, total_commits: commits.length, files: manyFiles(MAX_FILES_SHOWN + 5) })));

  const { payload } = await commitCompare({ repo: "o/r", base: "main", head: "x" });

  assert.equal(payload.commits.length, MAX_COMMITS_SHOWN);
  assert.equal(payload.commits[0].message, "c10");
  assert.equal(payload.commits.at(-1).message, `c${MAX_COMMITS_SHOWN + 9}`);
  assert.equal(payload.total_commits, MAX_COMMITS_SHOWN + 10);
  assert.equal(payload.files.length, MAX_FILES_SHOWN);
  assert.equal(payload.file_count, MAX_FILES_SHOWN + 5);
  assert.equal(payload.truncated, true);
  expectOutput(compare, payload);
});

test("commit_compare reports a missing ref and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }), reply(200, "nope"));

  await assert.rejects(commitCompare({ repo: "o/r", base: "main", head: "ghost" }), isRefusal(/could not find what it needs to compare 'ghost' with 'main' in o\/r \(404\)/));
  await assert.rejects(commitCompare({ repo: "o/r", base: "main", head: "x" }), isRefusal(/the body was not a comparison this server recognizes/));
  assert.equal(calls.length, 2);

  await assert.rejects(commitCompare({ repo: "o/r", head: "x" }), isRefusal(/`base` is required/));
  await assert.rejects(commitCompare({ repo: "o/r", base: "main" }), isRefusal(/`head` is required/));
  await assert.rejects(commitCompare({ repo: "o/r", base: "main", head: "a...b" }), isRefusal(/`head` 'a\.\.\.b' is not a valid/));
  assert.equal(calls.length, 2);
});
