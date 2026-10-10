import assert from "node:assert/strict";
import test from "node:test";
import { TOKEN_ENV } from "../src/config.js";
import { Refused } from "../src/errors.js";
import { TOOL as create, handle as repoCreate } from "../src/tools/repo-create.js";
import { VERSION } from "../src/version.js";
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
  assert.equal(TOKEN_ENV, "LXAGENTS_MCP_GITHUB_API_KEY");
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

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^LXAGENTS_MCP_GITHUB_API_KEY is not set\./));
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
    isRefusal(/^GitHub rejected the token \(401\)\. Check that LXAGENTS_MCP_GITHUB_API_KEY holds a valid token/),
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
