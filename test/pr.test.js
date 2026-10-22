import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as approve, handle as prApprove } from "../src/controllers/pr-approve.js";
import { TOOL as close, handle as prClose } from "../src/controllers/pr-close.js";
import { TOOL as comment, handle as prComment } from "../src/controllers/pr-comment.js";
import { TOOL as merge, handle as prMerge } from "../src/controllers/pr-merge.js";
import { TOOL as open, handle as prOpen } from "../src/controllers/pr-open.js";
import { TOOL as read, handle as prRead } from "../src/controllers/pr-read.js";
import { TOOL as request, handle as prReviewRequest } from "../src/controllers/pr-review-request.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const SHA = "0123456789abcdef0123456789abcdef01234567";

/** A pull request as GitHub returns it. */
const pull = (overrides = {}) => ({
  number: 7,
  title: "Add login",
  state: "open",
  draft: false,
  merged: false,
  mergeable: true,
  mergeable_state: "clean",
  user: { login: "ada" },
  head: { ref: "feat/login", sha: SHA },
  base: { ref: "main" },
  commits: 3,
  changed_files: 2,
  additions: 20,
  deletions: 4,
  labels: [{ name: "feature" }],
  requested_reviewers: [{ login: "bob" }],
  requested_teams: [{ slug: "platform" }],
  body: "Adds login",
  html_url: "https://github.com/o/r/pull/7",
  ...overrides,
});

/* ------------------------------------------------------------------ pr_open */

test("pr_open asks for the default branch when no base is given, then opens the pull request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(201, pull({ draft: true })));

  const { payload } = await prOpen({ repo: "o/r", title: " Add login ", head: "feat/login", body: "Why\n", draft: true });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r");
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/pulls");
  assert.deepEqual(calls[1].body, { title: "Add login", head: "feat/login", base: "main", body: "Why\n", draft: true });
  assert.deepEqual(payload, {
    summary: "Opened draft pull request #7 in o/r: feat/login into main.",
    number: 7,
    title: "Add login",
    state: "open",
    draft: true,
    head: "feat/login",
    base: "main",
    html_url: "https://github.com/o/r/pull/7",
  });
  expectOutput(open, payload);
});

test("pr_open with a base makes a single request, and takes a fork's branch as owner:branch", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, pull()));

  const { payload } = await prOpen({ repo: "o/r", title: "T", head: "ada:feat/x", base: "release/1.0" });

  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].body, { title: "T", head: "ada:feat/x", base: "release/1.0" });
  assert.equal(payload.summary, "Opened pull request #7 in o/r: ada:feat/x into release/1.0.");
});

test("pr_open explains a pull request that already exists, and a branch with nothing to propose", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(422, { message: "Validation Failed", errors: [{ message: "A pull request already exists for o:feat." }] }),
    reply(422, { message: "Validation Failed", errors: [{ message: "No commits between main and feat" }] }),
  );

  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "feat", base: "main" }), isRefusal(/\(422\).*A pull request already exists.*One is already open; find it with pr_read\.$/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "feat", base: "main" }), isRefusal(/No commits between main and feat.*The branches have no difference to propose\.$/));
});

test("pr_open refuses bad arguments before any request, and a repository whose default branch it cannot find", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(prOpen({ repo: "o/r", head: "x" }), isRefusal(/`title` is required/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T" }), isRefusal(/`head` is required/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "a b" }), isRefusal(/`head` 'a b' is not a valid/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "-bad:branch" }), isRefusal(/`head` '-bad:branch' is not 'branch' or 'owner:branch'/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "ada:a..b" }), isRefusal(/`head` 'a\.\.b' is not a valid/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "x", base: "a b" }), isRefusal(/`base` 'a b' is not a valid/));
  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "x", draft: "yes" }), isRefusal(/`draft` must be true or false/));
  assert.equal(calls.length, 0);

  await assert.rejects(prOpen({ repo: "o/r", title: "T", head: "x" }), isRefusal(/Could not find the default branch of o\/r; pass `base`/));
});

/* ------------------------------------------------------------------ pr_read */

test("pr_read reports the pull request in full", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, pull()));

  const { payload } = await prRead({ repo: "o/r", number: 7 });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/pulls/7");
  assert.deepEqual(payload, {
    summary: "Pull request #7 in o/r is open: Add login (feat/login into main).",
    number: 7,
    title: "Add login",
    state: "open",
    draft: false,
    merged: false,
    mergeable: true,
    mergeable_state: "clean",
    author: "ada",
    head: "feat/login",
    base: "main",
    head_sha: SHA,
    commits: 3,
    changed_files: 2,
    additions: 20,
    deletions: 4,
    labels: ["feature"],
    reviewers: ["bob", "team:platform"],
    html_url: "https://github.com/o/r/pull/7",
    body: "Adds login",
  });
  expectOutput(read, payload);
});

test("pr_read says a draft is a draft, a merged one is merged, and mergeability still being computed is not a no", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, pull({ draft: true })),
    reply(200, pull({ state: "closed", merged: true })),
    reply(200, pull({ mergeable: null, mergeable_state: "unknown" })),
  );

  assert.match((await prRead({ repo: "o/r", number: 7 })).payload.summary, /is a draft, open: Add login/);

  const merged = await prRead({ repo: "o/r", number: 7 });
  assert.match(merged.payload.summary, /is merged: Add login/);
  assert.equal(merged.payload.merged, true);

  const unknown = await prRead({ repo: "o/r", number: 7 });
  assert.equal("mergeable" in unknown.payload, false);
  assert.match(unknown.payload.summary, /Whether it can be merged is still being computed; ask again in a moment\.$/);
  expectOutput(read, unknown.payload);
});

test("pr_read lists the changed files, up to a hundred, when asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, pull()),
    reply(200, [
      { filename: "a.js", status: "added", additions: 10, deletions: 0, patch: "@@" },
      { filename: "b.js", status: "modified", additions: 10, deletions: 4 },
    ]),
  );

  const { payload } = await prRead({ repo: "o/r", number: 7, files: true });

  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/pulls/7/files?per_page=100");
  assert.deepEqual(payload.files, [
    { filename: "a.js", status: "added", additions: 10, deletions: 0 },
    { filename: "b.js", status: "modified", additions: 10, deletions: 4 },
  ]);
  expectOutput(read, payload);
});

test("pr_read reports a missing pull request and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }), reply(200, { nope: 1 }));

  await assert.rejects(prRead({ repo: "o/r", number: 9 }), isRefusal(/could not find what it needs to read pull request #9 of o\/r \(404\)/));
  await assert.rejects(prRead({ repo: "o/r", number: 9 }), isRefusal(/the body was not a pull request this server recognizes/));
  assert.equal(calls.length, 2);

  await assert.rejects(prRead({ repo: "o/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prRead({ repo: "o/r", number: 7, files: 1 }), isRefusal(/`files` must be true or false/));
  assert.equal(calls.length, 2);
});

/* ------------------------------------------------------------------ pr_comment */

test("pr_comment posts a conversation comment through the issue comments endpoint", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 5, html_url: "https://github.com/o/r/pull/7#issuecomment-5" }));

  const { payload } = await prComment({ repo: "o/r", number: 7, body: "Looks good\n" });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/7/comments");
  assert.deepEqual(calls[0].body, { body: "Looks good\n" });
  assert.deepEqual(payload, { summary: "Commented on pull request #7 in o/r.", id: 5, html_url: "https://github.com/o/r/pull/7#issuecomment-5" });
  expectOutput(comment, payload);
});

test("pr_comment says when the conversation is locked, and refuses a blank body before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(403, { message: "Unable to create comment because issue is locked." }));

  await assert.rejects(prComment({ repo: "o/r", number: 7, body: "x" }), isRefusal(/The conversation is locked\.$/));
  await assert.rejects(prComment({ repo: "o/r", number: 7, body: " " }), isRefusal(/`body` is required and must not be blank/));
  await assert.rejects(prComment({ repo: "o/r", body: "x" }), isRefusal(/`number` is required/));
  assert.equal(calls.length, 1);
});

/* ------------------------------------------------------------------ pr_close */

test("pr_close closes the pull request without merging", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, pull({ state: "closed" })));

  const { payload } = await prClose({ repo: "o/r", number: 7 });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "PATCH");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/pulls/7");
  assert.deepEqual(calls[0].body, { state: "closed" });
  assert.deepEqual(payload, {
    summary: "Closed pull request #7 in o/r without merging.",
    number: 7,
    state: "closed",
    merged: false,
    html_url: "https://github.com/o/r/pull/7",
  });
  expectOutput(close, payload);
});

test("pr_close comments first when asked, and says nothing was closed if it was already merged", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 1, html_url: "x" }), reply(200, pull({ state: "closed" })), reply(200, pull({ state: "closed", merged: true })));

  await prClose({ repo: "o/r", number: 7, comment: "Superseded by #8" });
  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/7/comments");
  assert.deepEqual(calls[0].body, { body: "Superseded by #8" });

  const merged = await prClose({ repo: "o/r", number: 7 });
  assert.equal(merged.payload.summary, "Pull request #7 in o/r was already merged; nothing to close.");
  assert.equal(merged.payload.merged, true);
});

test("pr_close says the comment was already posted when closing then fails, and refuses bad arguments", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 1, html_url: "x" }), reply(403, { message: "Must have push access" }));

  await assert.rejects(prClose({ repo: "o/r", number: 7, comment: "Bye" }), isRefusal(/close pull request #7 of o\/r \(403\).* The comment had already been posted\.$/));
  await assert.rejects(prClose({ repo: "o/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prClose({ repo: "o/r", number: 7, comment: 3 }), isRefusal(/`comment` must be a string/));
  assert.equal(calls.length, 2);
});

/* ------------------------------------------------------------------ pr_merge */

test("pr_merge merges by merge commit by default", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { sha: SHA, merged: true, message: "Pull Request successfully merged" }));

  const { payload } = await prMerge({ repo: "o/r", number: 7 });

  assert.equal(calls[0].method, "PUT");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/pulls/7/merge");
  assert.deepEqual(calls[0].body, { merge_method: "merge" });
  assert.deepEqual(payload, {
    summary: "Merged pull request #7 in o/r by merge, at 0123456.",
    merged: true,
    sha: SHA,
    message: "Pull Request successfully merged",
  });
  expectOutput(merge, payload);
});

test("pr_merge passes the method, the commit text and the head sha it must match", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { sha: SHA, merged: true }));

  await prMerge({ repo: "o/r", number: 7, method: "squash", title: " Add login (#7) ", message: "Body\n", sha: SHA });

  assert.deepEqual(calls[0].body, { merge_method: "squash", commit_title: "Add login (#7)", commit_message: "Body\n", sha: SHA });
});

test("pr_merge explains a pull request that is not mergeable, and a branch that moved", async (t) => {
  withToken(t);
  mockApi(t, reply(405, { message: "Pull Request is not mergeable" }), reply(409, { message: "Head branch was modified. Review and try the merge again." }));

  await assert.rejects(
    prMerge({ repo: "o/r", number: 7, method: "rebase" }),
    isRefusal(/^GitHub would not merge #7 in o\/r \(405\): Pull Request is not mergeable\. It is not mergeable: .*rebase merges may be disabled.*merge queue cannot be merged this way\.$/),
  );
  await assert.rejects(prMerge({ repo: "o/r", number: 7, sha: SHA }), isRefusal(/\(409\): Head branch was modified.*read the pull request again with pr_read\.$/));
});

test("pr_merge reports a 200 that did not merge as a failure, with GitHub's words", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { merged: false, message: "Merge queued" }));

  const { payload, ok } = await prMerge({ repo: "o/r", number: 7 });

  assert.equal(ok, false);
  assert.equal(payload.merged, false);
  assert.equal(payload.summary, "GitHub did not merge #7 in o/r: Merge queued");
  expectOutput(merge, payload);
});

test("pr_merge refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(prMerge({ repo: "o/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prMerge({ repo: "o/r", number: 7, method: "fast-forward" }), isRefusal(/`method` must be one of merge, squash, rebase/));
  await assert.rejects(prMerge({ repo: "o/r", number: 7, message: 4 }), isRefusal(/`message` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ pr_review_request */

test("pr_review_request asks people and teams and reports who is now requested", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, pull({ requested_reviewers: [{ login: "bob" }, { login: "cy" }], requested_teams: [{ slug: "platform" }] })));

  const { payload } = await prReviewRequest({ repo: "o/r", number: 7, reviewers: ["bob", "cy"], team_reviewers: ["platform"] });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/pulls/7/requested_reviewers");
  assert.deepEqual(calls[0].body, { reviewers: ["bob", "cy"], team_reviewers: ["platform"] });
  assert.deepEqual(payload, {
    summary: "Review requested on #7 in o/r from: bob, cy, team platform.",
    number: 7,
    reviewers: ["bob", "cy"],
    teams: ["platform"],
  });
  expectOutput(request, payload);
});

test("pr_review_request sends only the side that was given", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, pull({ requested_teams: [] })), reply(201, pull({ requested_reviewers: [] })));

  await prReviewRequest({ repo: "o/r", number: 7, reviewers: ["bob"] });
  assert.deepEqual(calls[0].body, { reviewers: ["bob"] });

  const teams = await prReviewRequest({ repo: "o/r", number: 7, team_reviewers: ["platform"] });
  assert.deepEqual(calls[1].body, { team_reviewers: ["platform"] });
  assert.deepEqual(teams.payload.reviewers, []);
});

test("pr_review_request needs at least one reviewer and explains a refused one", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(422, { message: "Review cannot be requested from pull request author." }));

  await assert.rejects(prReviewRequest({ repo: "o/r", number: 7 }), isRefusal(/^Give at least one of `reviewers` and `team_reviewers`\.$/));
  await assert.rejects(prReviewRequest({ repo: "o/r", number: 7, reviewers: [] }), isRefusal(/Give at least one/));
  await assert.rejects(prReviewRequest({ repo: "o/r", number: 7, reviewers: "bob" }), isRefusal(/`reviewers` must be a list of strings/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    prReviewRequest({ repo: "o/r", number: 7, reviewers: ["ada"] }),
    isRefusal(/\(422\): Review cannot be requested from pull request author\.\. Reviewers must be collaborators with read access/),
  );
});

/* ------------------------------------------------------------------ pr_approve */

test("pr_approve records an approving review", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { id: 31, state: "APPROVED", html_url: "https://github.com/o/r/pull/7#pullrequestreview-31" }));

  const { payload } = await prApprove({ repo: "o/r", number: 7 });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/pulls/7/reviews");
  assert.deepEqual(calls[0].body, { event: "APPROVE" });
  assert.deepEqual(payload, {
    summary: "Approved pull request #7 in o/r (review state APPROVED).",
    id: 31,
    state: "APPROVED",
    html_url: "https://github.com/o/r/pull/7#pullrequestreview-31",
  });
  expectOutput(approve, payload);
});

test("pr_approve passes a message and the head commit being approved", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { id: 32, state: "APPROVED" }));

  const { payload } = await prApprove({ repo: "o/r", number: 7, body: "LGTM", sha: SHA });

  assert.deepEqual(calls[0].body, { event: "APPROVE", body: "LGTM", commit_id: SHA });
  assert.equal("html_url" in payload, false);
});

test("pr_approve explains an author approving their own pull request, and refuses bad arguments", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(422, { message: "Unprocessable Entity", errors: ["Can not approve your own pull request"] }), reply(200, { nope: 1 }));

  await assert.rejects(prApprove({ repo: "o/r", number: 7 }), isRefusal(/\(422\): Unprocessable Entity \(Can not approve your own pull request\)\. A person cannot approve their own pull request; it needs a different reviewer\.$/));
  await assert.rejects(prApprove({ repo: "o/r", number: 7 }), isRefusal(/the body was not a review this server recognizes/));
  assert.equal(calls.length, 2);

  await assert.rejects(prApprove({ repo: "o/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prApprove({ repo: "o/r", number: 7, body: 3 }), isRefusal(/`body` must be a string/));
  assert.equal(calls.length, 2);
});
