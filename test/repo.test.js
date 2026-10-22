import assert from "node:assert/strict";
import test from "node:test";
import { TOKEN_ENV } from "../src/config/config.js";
import { Refused } from "../src/utils/errors.js";
import { TOOL as create, handle as repoCreate } from "../src/controllers/repo-create.js";
import { TOOL as del, handle as repoDelete } from "../src/controllers/repo-delete.js";
import { TOOL as fork, handle as repoFork } from "../src/controllers/repo-fork.js";
import { TOOL as read, handle as repoRead } from "../src/controllers/repo-read.js";
import { TOOL as search, handle as repoSearch } from "../src/controllers/repo-search.js";
import { TOOL as update, handle as repoUpdate } from "../src/controllers/repo-update.js";
import { VERSION } from "../src/config/version.js";
import { TOKEN, expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

/* ------------------------------------------------------------------ repo_create
 * Moved from the `cli` server, where it was `create_github_repo`. The behaviour is unchanged,
 * so these are that tool's tests with the new name and the shared helpers.
 */

const created = (overrides = {}) => ({
  name: "demo",
  full_name: "me/demo",
  html_url: "https://github.com/me/demo",
  clone_url: "https://github.com/me/demo.git",
  private: true,
  visibility: "private",
  default_branch: "main",
  ...overrides,
});

test("repo_create: the token variable is the one the owner named", () => {
  assert.equal(TOKEN_ENV, "GITHUB_API_KEY");
  assert.equal(create.name, "repo_create");
  assert.ok(create.description.includes(TOKEN_ENV));
  assert.deepEqual(Object.keys(create.inputSchema), ["name", "org", "description", "private", "auto_init"]);
});

test("repo_create: a repository is created in the token owner's account, private and initialized by default", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, created()));

  const { payload, ok } = await repoCreate({ name: "demo" });

  assert.equal(ok, true);
  assert.deepEqual(payload, {
    summary: "Created private repository me/demo on GitHub.",
    name: "demo",
    full_name: "me/demo",
    html_url: "https://github.com/me/demo",
    clone_url: "https://github.com/me/demo.git",
    visibility: "private",
    default_branch: "main",
  });
  expectOutput(create, payload);

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://api.github.com/user/repos");
  assert.equal(calls[0].method, "POST");
  assert.deepEqual(calls[0].body, { name: "demo", private: true, auto_init: true });
  assert.equal(calls[0].headers.Authorization, `Bearer ${TOKEN}`);
  assert.equal(calls[0].headers.Accept, "application/vnd.github+json");
  assert.equal(calls[0].headers["X-GitHub-Api-Version"], "2022-11-28");
  assert.equal(calls[0].headers["User-Agent"], `lxagents-mcp-github/${VERSION}`);
  assert.equal(calls[0].headers["Content-Type"], "application/json");
});

test("repo_create: an organization changes the endpoint, and the options reach GitHub", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, created({ full_name: "acme/demo", private: false, visibility: "public" })));

  const { payload } = await repoCreate({
    name: "demo",
    org: "acme",
    description: "  A demo  ",
    private: false,
    auto_init: false,
  });

  assert.equal(payload.summary, "Created public repository acme/demo on GitHub.");
  assert.equal(calls[0].url, "https://api.github.com/orgs/acme/repos");
  assert.deepEqual(calls[0].body, {
    name: "demo",
    private: false,
    auto_init: false,
    description: "A demo",
  });
});

test("repo_create: a blank org means the personal account", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, created()));

  await repoCreate({ name: "demo", org: "   " });

  assert.equal(calls[0].url, "https://api.github.com/user/repos");
});

test("repo_create: visibility falls back to the private flag, and a missing default branch is left out", async (t) => {
  withToken(t);
  mockApi(t, reply(201, created({ visibility: undefined, private: false, default_branch: undefined })));

  const { payload } = await repoCreate({ name: "demo", private: false });

  assert.equal(payload.visibility, "public");
  assert.equal("default_branch" in payload, false);
  expectOutput(create, payload);
});

test("repo_create: a missing token is a refusal that names the variable, and nothing is sent", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, created()));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^GITHUB_API_KEY is not set\./));
  assert.equal(calls.length, 0);
});

test("repo_create: bad arguments are refused before the token is read or anything is sent", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, created()));

  await assert.rejects(repoCreate({}), isRefusal(/`name` is required/));
  await assert.rejects(repoCreate({ name: "   " }), isRefusal(/`name` is required/));
  await assert.rejects(repoCreate({ name: 5 }), isRefusal(/`name` must be a string/));
  await assert.rejects(repoCreate({ name: "has space" }), isRefusal(/not a valid GitHub repository name/));
  await assert.rejects(repoCreate({ name: "a/b" }), isRefusal(/not a valid GitHub repository name/));
  await assert.rejects(repoCreate({ name: ".." }), isRefusal(/not a valid GitHub repository name/));
  await assert.rejects(repoCreate({ name: "a".repeat(101) }), isRefusal(/not a valid GitHub repository name/));
  await assert.rejects(repoCreate({ name: "ok", org: "bad/org" }), isRefusal(/not a valid GitHub organization name/));
  await assert.rejects(repoCreate({ name: "ok", org: "-".repeat(40) }), isRefusal(/not a valid GitHub organization name/));
  await assert.rejects(repoCreate({ name: "ok", private: "no" }), isRefusal(/`private` must be true or false/));
  await assert.rejects(repoCreate({ name: "ok", auto_init: 1 }), isRefusal(/`auto_init` must be true or false/));
  await assert.rejects(repoCreate({ name: "ok", description: 3 }), isRefusal(/`description` must be a string/));

  assert.equal(calls.length, 0);
});

test("repo_create: 401 says the token was rejected", async (t) => {
  withToken(t);
  mockApi(t, reply(401, { message: "Bad credentials" }));

  await assert.rejects(
    repoCreate({ name: "demo" }),
    isRefusal(/^GitHub rejected the token \(401\)\. Check that GITHUB_API_KEY holds a valid token/),
  );
});

test("repo_create: 403 says what permission the token needs, for the account or the organization", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { message: "Resource not accessible by personal access token" }), reply(403, { message: "no" }));

  await assert.rejects(
    repoCreate({ name: "demo" }),
    isRefusal(/403\): Resource not accessible .* in the token owner's account: a classic token needs the `repo` scope/),
  );
  await assert.rejects(repoCreate({ name: "demo", org: "acme" }), isRefusal(/repositories in 'acme'/));
});

test("repo_create: 404 on an organization says the organization may not exist", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }), reply(404, { message: "Not Found" }));

  await assert.rejects(
    repoCreate({ name: "demo", org: "ghost" }),
    isRefusal(/could not find the organization 'ghost' \(404\).*omit `org`/),
  );
  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^GitHub answered 404: Not Found\.$/));
});

test("repo_create: 422 carries GitHub's reason, such as a name that is taken", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(422, {
      message: "Repository creation failed.",
      errors: [{ resource: "Repository", code: "custom", field: "name", message: "name already exists on this account" }],
    }),
  );

  await assert.rejects(
    repoCreate({ name: "demo" }),
    isRefusal(
      /could not create 'demo' \(422\): Repository creation failed\. \(name already exists on this account\)\..*already exists in the token owner's account/,
    ),
  );
});

test("repo_create: any other status is reported with its status and message", async (t) => {
  withToken(t);
  mockApi(t, reply(500, { message: "Server Error" }));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^GitHub answered 500: Server Error\.$/));
});

test("repo_create: a failure never carries the token, even when GitHub echoes it", async (t) => {
  withToken(t);
  mockApi(t, reply(500, { message: `bad token ${TOKEN}` }));

  await assert.rejects(
    repoCreate({ name: "demo" }),
    (e) => e instanceof Refused && !e.message.includes(TOKEN) && e.message.includes("***"),
  );
});

test("repo_create: a 201 whose body is not a repository is refused", async (t) => {
  withToken(t);
  mockApi(t, reply(201, { unexpected: true }));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/answered 201, but the body was not a repository/));
});

test("repo_create: a network failure is a refusal", async (t) => {
  withToken(t);
  t.mock.method(globalThis, "fetch", async () => {
    throw new TypeError("fetch failed", { cause: new Error("ECONNREFUSED") });
  });

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^Could not reach api\.github\.com: ECONNREFUSED\./));
});

/* ------------------------------------------------------------------ the other repository tools */

/** A repository as GitHub returns it. */
const repository = (overrides = {}) => ({
  full_name: "o/r",
  description: "A thing",
  html_url: "https://github.com/o/r",
  clone_url: "https://github.com/o/r.git",
  private: false,
  visibility: "public",
  default_branch: "main",
  fork: false,
  archived: false,
  stargazers_count: 5,
  forks_count: 2,
  open_issues_count: 3,
  language: "JavaScript",
  license: { spdx_id: "MIT" },
  topics: ["mcp"],
  pushed_at: "2026-10-01T00:00:00Z",
  ...overrides,
});

const FACTS = {
  full_name: "o/r",
  description: "A thing",
  html_url: "https://github.com/o/r",
  clone_url: "https://github.com/o/r.git",
  visibility: "public",
  default_branch: "main",
  fork: false,
  archived: false,
  stars: 5,
  forks: 2,
  open_issues: 3,
  language: "JavaScript",
  license: "MIT",
  topics: ["mcp"],
  pushed_at: "2026-10-01T00:00:00Z",
};

test("repo_read reads one repository and reports its facts", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, repository()));

  const { payload, ok } = await repoRead({ repo: "o/r" });

  assert.equal(ok, true);
  assert.deepEqual(payload, { summary: "o/r is a public repository, default branch main.", ...FACTS });
  expectOutput(read, payload);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r");
});

test("repo_read says when a repository is a private archived fork, and leaves out what GitHub did not give", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(
      200,
      repository({
        visibility: undefined,
        private: true,
        fork: true,
        archived: true,
        description: null,
        language: null,
        license: null,
        topics: undefined,
        pushed_at: undefined,
        parent: { full_name: "up/stream" },
      }),
    ),
  );

  const { payload } = await repoRead({ repo: "o/r" });

  assert.equal(payload.summary, "o/r is a private archived fork, default branch main.");
  assert.equal(payload.parent, "up/stream");
  assert.deepEqual(payload.topics, []);
  for (const absent of ["description", "language", "license", "pushed_at"]) {
    assert.equal(absent in payload, false, absent);
  }
  expectOutput(read, payload);
});

test("repo_read refuses a bad repo before any request, a 404 with the cause, and an unexpected body", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }), reply(200, { nope: true }));

  await assert.rejects(repoRead({ repo: "nope" }), isRefusal(/`repo` 'nope' is not a repository/));
  await assert.rejects(repoRead({}), isRefusal(/`repo` is required/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    repoRead({ repo: "o/r" }),
    isRefusal(/^GitHub could not find what it needs to read the repository o\/r \(404\): Not Found\. .*private repository/),
  );
  await assert.rejects(repoRead({ repo: "o/r" }), isRefusal(/the body was not a repository this server recognizes/));
});

test("repo_update sends only what changes, and says what it changed", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, repository({ description: "New", archived: true })));

  const { payload } = await repoUpdate({ repo: "o/r", description: "New", archived: true, private: false });

  assert.equal(calls[0].method, "PATCH");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r");
  assert.deepEqual(calls[0].body, { description: "New", private: false, archived: true });
  assert.equal(payload.summary, "Updated o/r: description, private, archived.");
  assert.equal(payload.archived, true);
  expectOutput(update, payload);
});

test("repo_update can clear a description and a homepage with an empty string, and rename", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, repository({ full_name: "o/renamed", description: null })));

  const { payload } = await repoUpdate({ repo: "o/r", description: "", homepage: "", name: " renamed " });

  assert.deepEqual(calls[0].body, { name: "renamed", description: "", homepage: "" });
  assert.equal(payload.full_name, "o/renamed");
  assert.equal("description" in payload, false);
});

test("repo_update can change the default branch and the issues switch", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, repository({ default_branch: "trunk" })));

  await repoUpdate({ repo: "o/r", default_branch: "trunk", has_issues: false });

  assert.deepEqual(calls[0].body, { default_branch: "trunk", has_issues: false });
});

test("repo_update refuses an empty change and bad values before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, repository()));

  await assert.rejects(repoUpdate({ repo: "o/r" }), isRefusal(/^Nothing to change: pass at least one of name,/));
  await assert.rejects(repoUpdate({ repo: "o/r", name: "has space" }), isRefusal(/not a valid GitHub repository name/));
  await assert.rejects(repoUpdate({ repo: "o/r", name: ".." }), isRefusal(/not a valid GitHub repository name/));
  await assert.rejects(repoUpdate({ repo: "o/r", default_branch: "a..b" }), isRefusal(/`default_branch` 'a..b' is not a valid/));
  await assert.rejects(repoUpdate({ repo: "o/r", private: "yes" }), isRefusal(/`private` must be true or false/));
  await assert.rejects(repoUpdate({ repo: "o/r", archived: 1 }), isRefusal(/`archived` must be true or false/));
  await assert.rejects(repoUpdate({ repo: "o/r", description: 5 }), isRefusal(/`description` must be a string/));
  assert.equal(calls.length, 0);
});

test("repo_update carries GitHub's reason when it cannot change a setting", async (t) => {
  withToken(t);
  mockApi(t, reply(422, { message: "Validation Failed", errors: [{ message: "Visibility change not allowed" }] }));

  await assert.rejects(
    repoUpdate({ repo: "o/r", private: false }),
    isRefusal(/^GitHub could not update the repository o\/r \(422\): Validation Failed \(Visibility change not allowed\)\.$/),
  );
});

test("repo_delete deletes only when confirm repeats the full name, ignoring case", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  const { payload } = await repoDelete({ repo: "o/r", confirm: " O/R " });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r");
  assert.deepEqual(payload, { summary: "Deleted repository o/r.", deleted: true, full_name: "o/r" });
  expectOutput(del, payload);
});

test("repo_delete refuses without a matching confirm, and sends nothing", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  await assert.rejects(
    repoDelete({ repo: "o/r", confirm: "o/other" }),
    isRefusal(/^`confirm` must repeat the repository's full name, 'o\/r', to delete it\. Nothing was deleted\.$/),
  );
  await assert.rejects(repoDelete({ repo: "o/r", confirm: "r" }), isRefusal(/Nothing was deleted/));
  await assert.rejects(repoDelete({ repo: "o/r" }), isRefusal(/`confirm` is required/));
  await assert.rejects(repoDelete({ repo: "o/r", confirm: true }), isRefusal(/`confirm` must be a string/));
  await assert.rejects(repoDelete({ confirm: "o/r" }), isRefusal(/`repo` is required/));
  assert.equal(calls.length, 0);
});

test("repo_delete says what permission deleting needs, and that a missing repository is a 404", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { message: "Must have admin rights to Repository." }), reply(404, { message: "Not Found" }));

  await assert.rejects(
    repoDelete({ repo: "o/r", confirm: "o/r" }),
    isRefusal(/^GitHub refused to delete o\/r \(403\): Must have admin rights to Repository\.\. Deleting needs admin rights .*`delete_repo` scope/),
  );
  await assert.rejects(repoDelete({ repo: "o/r", confirm: "o/r" }), isRefusal(/could not find what it needs to delete the repository o\/r \(404\)/));
});

test("repo_fork forks into the token owner's account by default, and says GitHub finishes it later", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(202, repository({ full_name: "me/r", fork: true, parent: { full_name: "o/r" } })));

  const { payload } = await repoFork({ repo: "o/r" });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/forks");
  assert.deepEqual(calls[0].body, {});
  assert.match(payload.summary, /^Forking o\/r into me\/r has started\. GitHub finishes the copy in the background/);
  assert.equal(payload.parent, "o/r");
  assert.equal(payload.fork, true);
  expectOutput(fork, payload);
});

test("repo_fork passes the organization, the new name and the branch choice", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(202, repository({ full_name: "acme/copy", fork: true })));

  await repoFork({ repo: "o/r", organization: " acme ", name: "copy", default_branch_only: true });

  assert.deepEqual(calls[0].body, { organization: "acme", name: "copy", default_branch_only: true });
});

test("repo_fork refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(202, repository()));

  await assert.rejects(repoFork({ repo: "nope" }), isRefusal(/is not a repository/));
  await assert.rejects(repoFork({ repo: "o/r", name: "a b" }), isRefusal(/`name` 'a b' is not a valid GitHub repository name/));
  await assert.rejects(repoFork({ repo: "o/r", default_branch_only: "yes" }), isRefusal(/`default_branch_only` must be true or false/));
  assert.equal(calls.length, 0);
});

test("repo_search sends the query and the paging, and reports each match", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(
      200,
      {
        total_count: 2,
        incomplete_results: false,
        items: [
          { full_name: "a/mcp", description: "First", stargazers_count: 9, language: "Go", html_url: "https://github.com/a/mcp" },
          { full_name: "b/mcp", description: null, stargazers_count: 1, language: null, html_url: "https://github.com/b/mcp" },
        ],
      },
      { link: '<https://api.github.com/search/repositories?page=2>; rel="next"' },
    ),
  );

  const { payload } = await repoSearch({ query: " mcp in:name ", sort: "stars", order: "asc", per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(url.origin + url.pathname, "https://api.github.com/search/repositories");
  assert.deepEqual(Object.fromEntries(url.searchParams), { q: "mcp in:name", sort: "stars", order: "asc", per_page: "2", page: "1" });
  assert.deepEqual(payload, {
    summary: "2 repositories match 'mcp in:name'; showing 2.",
    items: [
      { full_name: "a/mcp", description: "First", stars: 9, language: "Go", url: "https://github.com/a/mcp" },
      { full_name: "b/mcp", stars: 1, url: "https://github.com/b/mcp" },
    ],
    total_count: 2,
    incomplete: false,
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(search, payload);
});

test("repo_search leaves order out unless there is a sort, and reports no match plainly", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { total_count: 0, incomplete_results: true, items: [] }));

  const { payload } = await repoSearch({ query: "zzzz", order: "asc" });

  assert.equal(new URL(calls[0].url).searchParams.has("order"), false);
  assert.equal(new URL(calls[0].url).searchParams.has("sort"), false);
  assert.equal(payload.summary, "No repositories match 'zzzz'.");
  assert.equal(payload.incomplete, true);
  assert.equal(payload.has_more, false);
  expectOutput(search, payload);
});

test("repo_search refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, { items: [] }));

  await assert.rejects(repoSearch({}), isRefusal(/`query` is required/));
  await assert.rejects(repoSearch({ query: "  " }), isRefusal(/`query` is required/));
  await assert.rejects(repoSearch({ query: "x", sort: "best" }), isRefusal(/`sort` must be one of stars, forks, help-wanted-issues, updated, got 'best'/));
  await assert.rejects(repoSearch({ query: "x", order: "up" }), isRefusal(/`order` must be one of asc, desc/));
  await assert.rejects(repoSearch({ query: "x", per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100/));
  assert.equal(calls.length, 0);
});

test("repo_search passes on GitHub's reason for a query it cannot run, and its rate limit", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(422, { message: "Validation Failed", errors: [{ message: "The listed users cannot be searched" }] }),
    reply(403, { message: "API rate limit exceeded" }, { "retry-after": "20" }),
  );

  await assert.rejects(repoSearch({ query: "user:ghost" }), isRefusal(/search repositories for 'user:ghost' \(422\): Validation Failed \(The listed users cannot be searched\)/));
  await assert.rejects(repoSearch({ query: "mcp" }), isRefusal(/rate limiting this token \(403\).*Wait 20 seconds/));
});
