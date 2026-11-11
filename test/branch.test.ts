import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as create, handle as branchCreate } from "../src/controllers/branch-create.js";
import { TOOL as del, handle as branchDelete } from "../src/controllers/branch-delete.js";
import { TOOL as list, handle as branchList } from "../src/controllers/branch-list.js";
import { TOOL as merge, handle as branchMerge } from "../src/controllers/branch-merge.js";
import { TOOL as read, handle as branchRead } from "../src/controllers/branch-read.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const SHA = "0123456789abcdef0123456789abcdef01234567";
const OTHER = "fedcba9876543210fedcba9876543210fedcba98";

test("branch_create finds the default branch's sha, then creates the ref", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, SHA), reply(201, { ref: "refs/heads/feat/x" }));

  const { payload } = await branchCreate({ repo: "o/r", branch: "feat/x" });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/commits/HEAD");
  assert.equal(calls[0].headers.Accept, "application/vnd.github.sha");
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/git/refs");
  assert.deepEqual(calls[1].body, { ref: "refs/heads/feat/x", sha: SHA });
  assert.deepEqual(payload, { summary: "Created branch feat/x at 0123456 in o/r.", branch: "feat/x", sha: SHA });
  expectOutput(create, payload);
});

test("branch_create can start from another branch, a tag or a sha", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, `${OTHER}\n`), reply(201, {}));

  const { payload } = await branchCreate({ repo: "o/r", branch: "hotfix", from: "release/1.0" });

  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/commits/release/1.0");
  assert.equal(calls[1].body.sha, OTHER);
  assert.equal(payload.from, "release/1.0");
  assert.equal(payload.summary, "Created branch hotfix at fedcba9 in o/r, from release/1.0.");
  expectOutput(create, payload);
});

test("branch_create says when the name is taken, when the source is missing, and when the repository is empty", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, SHA),
    reply(422, { message: "Reference already exists" }),
    reply(404, { message: "No commit found for the ref nope" }),
    reply(409, { message: "Git Repository is empty." }),
  );

  await assert.rejects(
    branchCreate({ repo: "o/r", branch: "main" }),
    isRefusal(/^GitHub could not create the branch 'main' \(422\): Reference already exists\. A branch with that name already exists in o\/r; pick another name\.$/),
  );
  await assert.rejects(
    branchCreate({ repo: "o/r", branch: "x", from: "nope" }),
    isRefusal(/could not find what it needs to find 'nope' in o\/r to branch from \(404\)/),
  );
  await assert.rejects(branchCreate({ repo: "o/r", branch: "x" }), isRefusal(/^o\/r is empty, so there is nothing to branch from\. Create its first commit with file_create\.$/));
});

test("branch_create refuses an answer that is not a sha, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, "<html>proxy error</html>"));

  await assert.rejects(branchCreate({ repo: "o/r", branch: "x" }), isRefusal(/the body was not a commit sha this server recognizes/));
  assert.equal(calls.length, 1);

  await assert.rejects(branchCreate({ repo: "o/r" }), isRefusal(/`branch` is required/));
  await assert.rejects(branchCreate({ repo: "o/r", branch: "a..b" }), isRefusal(/`branch` 'a\.\.b' is not a valid branch, tag or commit name/));
  await assert.rejects(branchCreate({ repo: "o/r", branch: "x", from: "a b" }), isRefusal(/`from` 'a b' is not a valid/));
  await assert.rejects(branchCreate({ repo: "nope", branch: "x" }), isRefusal(/is not a repository/));
  assert.equal(calls.length, 1);
});

test("branch_read reports the tip, its message and date, and whether the branch is protected", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, {
      name: "feat/x",
      protected: true,
      commit: { sha: SHA, commit: { message: "Add x\n\nLonger body", author: { date: "2026-10-01T12:00:00Z" } } },
    }),
  );

  const { payload } = await branchRead({ repo: "o/r", branch: "feat/x" });

  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/branches/feat/x");
  assert.deepEqual(payload, {
    summary: "Branch feat/x of o/r is at 0123456 and is protected.",
    name: "feat/x",
    sha: SHA,
    protected: true,
    message: "Add x",
    date: "2026-10-01T12:00:00Z",
  });
  expectOutput(read, payload);
});

test("branch_read leaves out what GitHub did not give, and refuses a missing branch plainly", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { name: "main", protected: false, commit: { sha: SHA } }), reply(404, { message: "Branch not found" }));

  const { payload } = await branchRead({ repo: "o/r", branch: "main" });
  assert.equal(payload.summary, "Branch main of o/r is at 0123456.");
  assert.equal("message" in payload, false);
  assert.equal("date" in payload, false);
  expectOutput(read, payload);

  await assert.rejects(branchRead({ repo: "o/r", branch: "gone" }), isRefusal(/could not find what it needs to read the branch 'gone' of o\/r \(404\): Branch not found/));
});

test("branch_read encodes odd characters in a name and refuses a name that is not one", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { name: "a#b", protected: false, commit: { sha: SHA } }));

  await branchRead({ repo: "o/r", branch: "a#b" });
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/branches/a%23b");

  await assert.rejects(branchRead({ repo: "o/r", branch: "../x" }), isRefusal(/is not a valid branch, tag or commit name/));
  await assert.rejects(branchRead({ repo: "o/r" }), isRefusal(/`branch` is required/));
  assert.equal(calls.length, 1);
});

test("branch_list sends the filter and the paging, and lists each branch", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(
      200,
      [
        { name: "main", commit: { sha: SHA }, protected: true },
        { name: "dev", commit: { sha: OTHER }, protected: false },
      ],
      { link: '<https://api.github.com/repositories/1/branches?page=3>; rel="next"' },
    ),
  );

  const { payload } = await branchList({ repo: "o/r", protected: true, page: 2, per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/repos/o/r/branches");
  assert.deepEqual(Object.fromEntries(url.searchParams), { protected: "true", page: "2", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 protected branches of o/r on page 2.",
    items: [
      { name: "main", sha: SHA, protected: true },
      { name: "dev", sha: OTHER, protected: false },
    ],
    page: 2,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("branch_list sends no filter by default, sends false when asked, and reports an empty page", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, []), reply(200, [{ name: "x", commit: { sha: SHA }, protected: false }]));

  const empty = await branchList({ repo: "o/r" });
  assert.equal(new URL(calls[0].url).searchParams.has("protected"), false);
  assert.equal(empty.payload.summary, "o/r has no branches on page 1.");
  assert.deepEqual(empty.payload.items, []);
  assert.equal(empty.payload.has_more, false);

  const one = await branchList({ repo: "o/r", protected: false });
  assert.equal(new URL(calls[1].url).searchParams.get("protected"), "false");
  assert.equal(one.payload.summary, "1 unprotected branch of o/r on page 1.");
});

test("branch_list refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(branchList({ repo: "o/r", protected: "yes" }), isRefusal(/`protected` must be true or false/));
  await assert.rejects(branchList({ repo: "o/r", page: 0 }), isRefusal(/`page` must be between 1 and/));
  await assert.rejects(branchList({}), isRefusal(/`repo` is required/));
  assert.equal(calls.length, 0);
});

test("branch_delete removes the ref and says what it removed", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  const { payload } = await branchDelete({ repo: "o/r", branch: "feat/x" });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/git/refs/heads/feat/x");
  assert.deepEqual(payload, { summary: "Deleted branch feat/x from o/r.", deleted: true, branch: "feat/x" });
  expectOutput(del, payload);
});

test("branch_delete explains a 422 as a missing branch or the default branch", async (t) => {
  withToken(t);
  mockApi(t, reply(422, { message: "Reference does not exist" }));

  await assert.rejects(
    branchDelete({ repo: "o/r", branch: "main" }),
    isRefusal(/^GitHub would not delete the branch 'main' from o\/r \(422\): Reference does not exist\. The branch may not exist, or it is the default branch/),
  );
});

test("branch_delete refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  await assert.rejects(branchDelete({ repo: "o/r" }), isRefusal(/`branch` is required/));
  await assert.rejects(branchDelete({ repo: "o/r", branch: "a/../b" }), isRefusal(/is not a valid/));
  assert.equal(calls.length, 0);
});

test("branch_merge merges the head into the base and reports the merge commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { sha: SHA, commit: { message: "Merge feat/x into main\n\nbody" } }));

  const { payload } = await branchMerge({ repo: "o/r", base: "main", head: "feat/x", message: " Ship it " });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/merges");
  assert.deepEqual(calls[0].body, { base: "main", head: "feat/x", commit_message: "Ship it" });
  assert.deepEqual(payload, {
    summary: "Merged feat/x into main in o/r, at 0123456.",
    merged: true,
    up_to_date: false,
    sha: SHA,
    message: "Merge feat/x into main",
  });
  expectOutput(merge, payload);
});

test("branch_merge sends no message by default, and treats a 204 as nothing to merge", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  const { payload } = await branchMerge({ repo: "o/r", base: "main", head: "feat/x" });

  assert.deepEqual(calls[0].body, { base: "main", head: "feat/x" });
  assert.deepEqual(payload, { summary: "Nothing to merge: main already contains feat/x.", merged: false, up_to_date: true });
  expectOutput(merge, payload);
});

test("branch_merge explains a conflict and a missing branch", async (t) => {
  withToken(t);
  mockApi(t, reply(409, { message: "Merge conflict" }), reply(404, { message: "Not Found" }));

  await assert.rejects(
    branchMerge({ repo: "o/r", base: "main", head: "feat/x" }),
    isRefusal(/^GitHub could not merge 'feat\/x' into 'main' in o\/r \(409\): Merge conflict\. The branches conflict\. Open a pull request/),
  );
  await assert.rejects(
    branchMerge({ repo: "o/r", base: "main", head: "ghost" }),
    isRefusal(/^GitHub could not find 'main' or 'ghost' in o\/r \(404\): Not Found\. Check both names/),
  );
});

test("branch_merge refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, {}));

  await assert.rejects(branchMerge({ repo: "o/r", head: "x" }), isRefusal(/`base` is required/));
  await assert.rejects(branchMerge({ repo: "o/r", base: "main" }), isRefusal(/`head` is required/));
  await assert.rejects(branchMerge({ repo: "o/r", base: "a b", head: "x" }), isRefusal(/`base` 'a b' is not a valid/));
  await assert.rejects(branchMerge({ repo: "o/r", base: "main", head: "x", message: 3 }), isRefusal(/`message` must be a string/));
  assert.equal(calls.length, 0);
});
