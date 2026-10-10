import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as assign, handle as issueAssign } from "../src/tools/issue-assign.js";
import { TOOL as close, handle as issueClose } from "../src/tools/issue-close.js";
import { TOOL as comment, handle as issueComment } from "../src/tools/issue-comment.js";
import { TOOL as label, handle as issueLabel } from "../src/tools/issue-label.js";
import { TOOL as lock, handle as issueLock } from "../src/tools/issue-lock.js";
import { TOOL as open, handle as issueOpen } from "../src/tools/issue-open.js";
import { TOOL as read, handle as issueRead } from "../src/tools/issue-read.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

/** An issue as GitHub returns it. */
const issue = (overrides = {}) => ({
  number: 42,
  title: "Login fails",
  state: "open",
  html_url: "https://github.com/o/r/issues/42",
  user: { login: "ada" },
  labels: [{ name: "bug" }],
  assignees: [{ login: "bob" }],
  milestone: { title: "v1" },
  comments: 2,
  created_at: "2026-10-01T00:00:00Z",
  updated_at: "2026-10-02T00:00:00Z",
  body: "Steps\nto reproduce",
  ...overrides,
});

/* ------------------------------------------------------------------ issue_open */

test("issue_open opens an issue with everything it was given", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, issue({ labels: [{ name: "bug" }, { name: "ui" }], assignees: [{ login: "bob" }] })));

  const { payload } = await issueOpen({
    repo: "o/r",
    title: " Login fails ",
    body: "Steps\n",
    labels: ["bug", "ui", "bug"],
    assignees: ["bob"],
    milestone: 3,
  });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues");
  assert.deepEqual(calls[0].body, { title: "Login fails", body: "Steps\n", labels: ["bug", "ui"], assignees: ["bob"], milestone: 3 });
  assert.deepEqual(payload, {
    summary: "Opened issue #42 in o/r: Login fails.",
    number: 42,
    title: "Login fails",
    state: "open",
    html_url: "https://github.com/o/r/issues/42",
    labels: ["bug", "ui"],
    assignees: ["bob"],
  });
  expectOutput(open, payload);
});

test("issue_open sends only the title by default, and says what GitHub silently dropped", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, issue({ labels: [], assignees: [] })), reply(201, issue({ labels: [{ name: "bug" }], assignees: [] })));

  const plain = await issueOpen({ repo: "o/r", title: "T" });
  assert.deepEqual(calls[0].body, { title: "T" });
  assert.equal(plain.payload.summary, "Opened issue #42 in o/r: Login fails.");

  const partial = await issueOpen({ repo: "o/r", title: "T", labels: ["bug", "nope"], assignees: ["ghost"] });
  assert.match(partial.payload.summary, /GitHub did not apply: label nope, assignee ghost\. The token may lack push access/);
  expectOutput(open, partial.payload);
});

test("issue_open points at repo_update when issues are disabled, and refuses bad arguments first", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(410, { message: "Issues are disabled for this repo" }));

  await assert.rejects(issueOpen({ repo: "o/r", title: "T" }), isRefusal(/^Issues are disabled in o\/r \(410\): Issues are disabled for this repo\. Turn them on with repo_update \(has_issues\)\.$/));
  assert.equal(calls.length, 1);

  await assert.rejects(issueOpen({ repo: "o/r" }), isRefusal(/`title` is required/));
  await assert.rejects(issueOpen({ repo: "o/r", title: "T", labels: "bug" }), isRefusal(/`labels` must be a list of strings/));
  await assert.rejects(issueOpen({ repo: "o/r", title: "T", milestone: 0 }), isRefusal(/`milestone` must be between 1 and/));
  await assert.rejects(issueOpen({ repo: "o/r", title: "T", body: 4 }), isRefusal(/`body` must be a string/));
  assert.equal(calls.length, 1);
});

/* ------------------------------------------------------------------ issue_read */

test("issue_read reports the issue, its labels, assignees and body", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ closed_at: "2026-10-03T00:00:00Z", state: "closed", state_reason: "completed" })));

  const { payload } = await issueRead({ repo: "o/r", number: 42 });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42");
  assert.deepEqual(payload, {
    summary: "Issue #42 in o/r is closed: Login fails.",
    number: 42,
    title: "Login fails",
    state: "closed",
    state_reason: "completed",
    author: "ada",
    labels: ["bug"],
    assignees: ["bob"],
    milestone: "v1",
    comment_count: 2,
    is_pull_request: false,
    html_url: "https://github.com/o/r/issues/42",
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-02T00:00:00Z",
    closed_at: "2026-10-03T00:00:00Z",
    body: "Steps\nto reproduce",
  });
  expectOutput(read, payload);
});

test("issue_read fetches the first thirty comments only when asked, and only if there are any", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, issue({ comments: 45 })),
    reply(200, [
      { id: 1, user: { login: "bob" }, created_at: "2026-10-02T01:00:00Z", body: "First\nline two" },
      { id: 2, user: null, body: null },
    ]),
    reply(200, issue({ comments: 0 })),
  );

  const withComments = await issueRead({ repo: "o/r", number: 42, comments: true });
  assert.equal(calls.length, 2);
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/issues/42/comments?per_page=30");
  assert.deepEqual(withComments.payload.comments, [
    { id: 1, author: "bob", created_at: "2026-10-02T01:00:00Z", body: "First\nline two" },
    { id: 2, body: "" },
  ]);
  assert.match(withComments.payload.summary, /; showing the first 2 comments\.$/);
  expectOutput(read, withComments.payload);

  const none = await issueRead({ repo: "o/r", number: 42, comments: true });
  assert.equal(calls.length, 3);
  assert.equal("comments" in none.payload, false);
});

test("issue_read says when the number is a pull request", async (t) => {
  withToken(t);
  mockApi(t, reply(200, issue({ pull_request: { url: "x" } })));

  const { payload } = await issueRead({ repo: "o/r", number: 42 });

  assert.equal(payload.is_pull_request, true);
  assert.match(payload.summary, /^Pull request #42 in o\/r is open/);
});

test("issue_read reports a missing issue and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }), reply(200, { nope: 1 }));

  await assert.rejects(issueRead({ repo: "o/r", number: 9 }), isRefusal(/could not find what it needs to read issue #9 of o\/r \(404\)/));
  await assert.rejects(issueRead({ repo: "o/r", number: 9 }), isRefusal(/the body was not an issue this server recognizes/));
  assert.equal(calls.length, 2);

  await assert.rejects(issueRead({ repo: "o/r" }), isRefusal(/`number` is required/));
  await assert.rejects(issueRead({ repo: "o/r", number: "42" }), isRefusal(/`number` must be a whole number/));
  await assert.rejects(issueRead({ repo: "o/r", number: 0 }), isRefusal(/`number` must be between 1 and/));
  await assert.rejects(issueRead({ repo: "o/r", number: 1, comments: "yes" }), isRefusal(/`comments` must be true or false/));
  assert.equal(calls.length, 2);
});

/* ------------------------------------------------------------------ issue_comment */

test("issue_comment posts the body as given", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 99, html_url: "https://github.com/o/r/issues/42#issuecomment-99" }));

  const { payload } = await issueComment({ repo: "o/r", number: 42, body: "  Thanks!\n" });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/comments");
  assert.deepEqual(calls[0].body, { body: "  Thanks!\n" });
  assert.deepEqual(payload, { summary: "Commented on #42 in o/r.", id: 99, html_url: "https://github.com/o/r/issues/42#issuecomment-99" });
  expectOutput(comment, payload);
});

test("issue_comment says when the conversation is locked, and refuses a blank body before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(403, { message: "Unable to create comment because issue is locked." }));

  await assert.rejects(issueComment({ repo: "o/r", number: 42, body: "x" }), isRefusal(/\(403\): Unable to create comment because issue is locked\.\. The conversation is locked\.$/));
  await assert.rejects(issueComment({ repo: "o/r", number: 42 }), isRefusal(/`body` is required/));
  await assert.rejects(issueComment({ repo: "o/r", number: 42, body: "  " }), isRefusal(/`body` is required and must not be blank/));
  assert.equal(calls.length, 1);
});

/* ------------------------------------------------------------------ issue_close */

test("issue_close closes as completed by default", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ state: "closed", state_reason: "completed" })));

  const { payload } = await issueClose({ repo: "o/r", number: 42 });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "PATCH");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42");
  assert.deepEqual(calls[0].body, { state: "closed", state_reason: "completed" });
  assert.deepEqual(payload, {
    summary: "Closed issue #42 in o/r as completed.",
    number: 42,
    state: "closed",
    state_reason: "completed",
    html_url: "https://github.com/o/r/issues/42",
  });
  expectOutput(close, payload);
});

test("issue_close can close as not planned, commenting first", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 1, html_url: "x" }), reply(200, issue({ state: "closed", state_reason: "not_planned" })));

  const { payload } = await issueClose({ repo: "o/r", number: 42, reason: "not_planned", comment: "Out of scope." });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/comments");
  assert.deepEqual(calls[0].body, { body: "Out of scope." });
  assert.equal(calls[1].body.state_reason, "not_planned");
  assert.equal(payload.summary, "Closed issue #42 in o/r as not planned.");
});

test("issue_close says the comment was already posted when closing then fails", async (t) => {
  withToken(t);
  mockApi(t, reply(201, { id: 1, html_url: "x" }), reply(403, { message: "Must have push access" }));

  await assert.rejects(issueClose({ repo: "o/r", number: 42, comment: "Bye" }), isRefusal(/close issue #42 of o\/r \(403\).* The comment had already been posted\.$/));
});

test("issue_close refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, issue()));

  await assert.rejects(issueClose({ repo: "o/r", number: 42, reason: "wontfix" }), isRefusal(/`reason` must be one of completed, not_planned, duplicate, got 'wontfix'/));
  await assert.rejects(issueClose({ repo: "o/r" }), isRefusal(/`number` is required/));
  await assert.rejects(issueClose({ repo: "o/r", number: 1, comment: 5 }), isRefusal(/`comment` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ issue_assign */

test("issue_assign adds assignees by default and reports who is assigned", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, issue({ assignees: [{ login: "bob" }, { login: "cy" }] })));

  const { payload } = await issueAssign({ repo: "o/r", number: 42, assignees: ["cy"] });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/assignees");
  assert.deepEqual(calls[0].body, { assignees: ["cy"] });
  assert.deepEqual(payload, { summary: "Assignees of #42 in o/r: bob, cy.", number: 42, assignees: ["bob", "cy"] });
  expectOutput(assign, payload);
});

test("issue_assign removes with a body on DELETE, and replaces with a PATCH", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ assignees: [] })), reply(200, issue({ assignees: [{ login: "ann" }] })));

  const removed = await issueAssign({ repo: "o/r", number: 42, assignees: ["bob"], action: "remove" });
  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/assignees");
  assert.deepEqual(calls[0].body, { assignees: ["bob"] });
  assert.equal(removed.payload.summary, "Assignees of #42 in o/r: none.");

  const set = await issueAssign({ repo: "o/r", number: 42, assignees: ["ann"], action: "set" });
  assert.equal(calls[1].method, "PATCH");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/issues/42");
  assert.deepEqual(set.payload.assignees, ["ann"]);
});

test("issue_assign reports the people GitHub silently did not assign or remove", async (t) => {
  withToken(t);
  mockApi(t, reply(201, issue({ assignees: [{ login: "bob" }] })), reply(200, issue({ assignees: [{ login: "bob" }] })));

  const add = await issueAssign({ repo: "o/r", number: 42, assignees: ["bob", "ghost"] });
  assert.deepEqual(add.payload.not_applied, ["ghost"]);
  assert.match(add.payload.summary, /GitHub did not assign: ghost\. They may not be collaborators/);
  expectOutput(assign, add.payload);

  const remove = await issueAssign({ repo: "o/r", number: 42, assignees: ["bob"], action: "remove" });
  assert.deepEqual(remove.payload.not_applied, ["bob"]);
  assert.match(remove.payload.summary, /GitHub did not remove: bob\./);
});

test("issue_assign refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, issue()));

  await assert.rejects(issueAssign({ repo: "o/r", number: 42 }), isRefusal(/`assignees` is required/));
  await assert.rejects(issueAssign({ repo: "o/r", number: 42, assignees: [] }), isRefusal(/`assignees` is required/));
  await assert.rejects(issueAssign({ repo: "o/r", number: 42, assignees: Array.from({ length: 11 }, (_, i) => `u${i}`) }), isRefusal(/at most 10 values, got 11/));
  await assert.rejects(issueAssign({ repo: "o/r", number: 42, assignees: ["a"], action: "toggle" }), isRefusal(/`action` must be one of add, remove, set/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ issue_label */

test("issue_label adds labels and reports every label now on the issue", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [{ name: "bug" }, { name: "good first issue" }]));

  const { payload } = await issueLabel({ repo: "o/r", number: 42, labels: ["good first issue"] });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/labels");
  assert.deepEqual(calls[0].body, { labels: ["good first issue"] });
  assert.deepEqual(payload, { summary: "Labels on #42 in o/r: bug, good first issue.", number: 42, labels: ["bug", "good first issue"] });
  expectOutput(label, payload);
});

test("issue_label replaces with PUT and reports a label GitHub did not apply", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [{ name: "bug" }]));

  const { payload } = await issueLabel({ repo: "o/r", number: 42, labels: ["bug", "nope"], action: "set" });

  assert.equal(calls[0].method, "PUT");
  assert.deepEqual(payload.not_applied, ["nope"]);
  assert.match(payload.summary, /GitHub did not apply: nope\.$/);
  expectOutput(label, payload);
});

test("issue_label removes one label at a time, encoding the name, and treats 404 as already off", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [{ name: "ui" }]), reply(404, { message: "Label does not exist" }));

  const { payload } = await issueLabel({ repo: "o/r", number: 42, labels: ["good first issue", "gone"], action: "remove" });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/labels/good%20first%20issue");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/issues/42/labels/gone");
  assert.deepEqual(payload, {
    summary: "Labels on #42 in o/r: ui. Not on the issue to begin with: gone.",
    number: 42,
    labels: ["ui"],
    not_applied: ["gone"],
  });
  expectOutput(label, payload);
});

test("issue_label reads the labels when every removal was a 404", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, {}), reply(200, [{ name: "bug" }]));

  const { payload } = await issueLabel({ repo: "o/r", number: 42, labels: ["x"], action: "remove" });

  assert.equal(calls[1].method, "GET");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/issues/42/labels");
  assert.deepEqual(payload.labels, ["bug"]);
});

test("issue_label refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(issueLabel({ repo: "o/r", number: 42 }), isRefusal(/`labels` is required/));
  await assert.rejects(issueLabel({ repo: "o/r", number: 42, labels: ["a"], action: "x" }), isRefusal(/`action` must be one of add, remove, set/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ issue_lock */

test("issue_lock locks with a reason", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  const { payload } = await issueLock({ repo: "o/r", number: 42, reason: "too heated" });

  assert.equal(calls[0].method, "PUT");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/issues/42/lock");
  assert.deepEqual(calls[0].body, { lock_reason: "too heated" });
  assert.deepEqual(payload, { summary: "Locked #42 in o/r as too heated.", number: 42, locked: true, reason: "too heated" });
  expectOutput(lock, payload);
});

test("issue_lock locks with no body when there is no reason, and unlocks", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined), reply(204, undefined));

  const locked = await issueLock({ repo: "o/r", number: 42 });
  assert.equal(calls[0].body, undefined);
  assert.equal(locked.payload.summary, "Locked #42 in o/r.");
  assert.equal("reason" in locked.payload, false);

  const unlocked = await issueLock({ repo: "o/r", number: 42, locked: false, reason: "spam" });
  assert.equal(calls[1].method, "DELETE");
  assert.deepEqual(unlocked.payload, { summary: "Unlocked #42 in o/r.", number: 42, locked: false });
  expectOutput(lock, unlocked.payload);
});

test("issue_lock refuses bad arguments before any request, and reports a permission it lacks", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(403, { message: "Must have push access" }));

  await assert.rejects(issueLock({ repo: "o/r", number: 42, reason: "boring" }), isRefusal(/`reason` must be one of off-topic, too heated, resolved, spam/));
  await assert.rejects(issueLock({ repo: "o/r", number: 42, locked: "yes" }), isRefusal(/`locked` must be true or false/));
  assert.equal(calls.length, 0);
  await assert.rejects(issueLock({ repo: "o/r", number: 42 }), isRefusal(/GitHub refused to lock #42 of o\/r \(403\): Must have push access/));
});
