import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import {
  apiDetail,
  callApi,
  filePath,
  github,
  pageQuery,
  paging,
  parseRepo,
  redact,
  refName,
  refPath,
  requireToken,
  workflowName,
} from "../src/services/api.js";
import { API_TIMEOUT_MS } from "../src/config/config.js";
import { Refused } from "../src/utils/errors.js";
import { VERSION } from "../src/config/version.js";
import { TOKEN, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

/** Set an environment variable for one test and put it back afterwards. */
function env(t, name, value) {
  const saved = process.env[name];
  if (value === undefined) delete process.env[name];
  else process.env[name] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[name];
    else process.env[name] = saved;
  });
}

/* ------------------------------------------------------------------ the token */

test("requireToken returns the trimmed GitHub key", (t) => {
  env(t, "GITHUB_API_KEY", "  abc123  ");

  assert.equal(requireToken(), "abc123");
});

test("requireToken refuses a missing or blank token and names the variable", (t) => {
  env(t, "GITHUB_API_KEY", undefined);
  assert.throws(() => requireToken(), isRefusal(/^GITHUB_API_KEY is not set\. .*`env` block/));

  env(t, "GITHUB_API_KEY", "   ");
  assert.throws(() => requireToken(), isRefusal(/GITHUB_API_KEY is not set/));
});

test("requireToken reads the GitHub variable by default", (t) => {
  withoutToken(t);
  assert.throws(() => requireToken(), isRefusal(/^GITHUB_API_KEY is not set\./));
});

test("redact removes every occurrence of a secret and tolerates an empty one", () => {
  assert.equal(redact("a s3cret b s3cret", "s3cret"), "a *** b ***");
  assert.equal(redact("nothing here", "s3cret"), "nothing here");
  assert.equal(redact("kept", ""), "kept");
});

/* ------------------------------------------------------------------ callApi */

test("callApi sends the method, headers and a JSON body, and decodes the answer", async (t) => {
  const fetch = t.mock.method(globalThis, "fetch", async () => new Response('{"ok":true}', { status: 201 }));

  const result = await callApi("https://api.example.com/things", {
    method: "POST",
    headers: { Authorization: "Bearer x" },
    json: { a: 1 },
  });

  assert.equal(result.status, 201);
  assert.deepEqual(result.body, { ok: true });
  assert.equal(typeof result.headers, "object");
  const [url, init] = fetch.mock.calls[0].arguments;
  assert.equal(url, "https://api.example.com/things");
  assert.equal(init.method, "POST");
  assert.deepEqual(init.headers, { Authorization: "Bearer x", "Content-Type": "application/json" });
  assert.equal(init.body, '{"a":1}');
  assert.ok(init.signal instanceof AbortSignal);
});

test("callApi hands back the response headers, lowercased", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("[]", { status: 200, headers: { Link: '<x>; rel="next"' } }));

  const { headers } = await callApi("https://api.example.com/things");

  assert.equal(headers.link, '<x>; rel="next"');
});

test("callApi sends no body and no content type when there is no JSON", async (t) => {
  const fetch = t.mock.method(globalThis, "fetch", async () => new Response(null, { status: 204 }));

  const result = await callApi("https://api.example.com/things");

  assert.equal(result.status, 204);
  assert.equal(result.body, null);
  const [, init] = fetch.mock.calls[0].arguments;
  assert.equal(init.method, "GET");
  assert.equal(init.body, undefined);
  assert.deepEqual(init.headers, {});
});

test("callApi hands back a body that is not JSON as short text", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("x".repeat(1000), { status: 502 }));

  const { status, body } = await callApi("https://api.example.com/things");

  assert.equal(status, 502);
  assert.equal(body, "x".repeat(300));
});

test("callApi keeps the whole body as text when asked for it raw", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("y".repeat(1000), { status: 200 }));

  const { body } = await callApi("https://api.example.com/log", { raw: true });

  assert.equal(body, "y".repeat(1000));
});

test("callApi still decodes an error answer when the body of a good one is asked for raw", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(JSON.stringify({ message: "Not Found" }), { status: 404 }));

  const { status, body } = await callApi("https://api.example.com/log", { raw: true });

  assert.equal(status, 404);
  assert.deepEqual(body, { message: "Not Found" });
});

test("callApi follows a redirect to another host without sending the token there", async (t) => {
  // A job's log is fetched through a redirect from GitHub to a signed address on another host. The
  // token is for GitHub alone; this holds the client to dropping it on the hop, over real sockets.
  const seen = [];
  const target = createServer((request, response) => {
    seen.push(request.headers.authorization ?? null);
    response.end("the log");
  });
  await new Promise((resolve) => target.listen(0, "127.0.0.1", resolve));
  const origin = createServer((request, response) => {
    response.writeHead(302, { location: `http://127.0.0.1:${target.address().port}/blob` });
    response.end();
  });
  await new Promise((resolve) => origin.listen(0, "127.0.0.1", resolve));
  t.after(() => {
    for (const server of [origin, target]) {
      server.closeAllConnections();
      server.close();
    }
  });

  const { status, body } = await callApi(`http://127.0.0.1:${origin.address().port}/logs`, {
    raw: true,
    headers: { Authorization: "Bearer secret" },
  });

  assert.equal(status, 200);
  assert.equal(body, "the log");
  assert.deepEqual(seen, [null]);
});

test("callApi refuses an answer larger than the client will read", async (t) => {
  t.mock.method(
    globalThis,
    "fetch",
    async () =>
      new Response(
        new ReadableStream({
          pull(controller) {
            controller.enqueue(new Uint8Array(5_000_000));
          },
        }),
        { status: 200 },
      ),
  );

  await assert.rejects(callApi("https://api.example.com/big", { raw: true }), isRefusal(/larger than 20 MB/));
});

test("callApi turns a network failure into a refusal with the secret scrubbed", async (t) => {
  t.mock.method(globalThis, "fetch", async () => {
    throw new TypeError("fetch failed", { cause: new Error("getaddrinfo ENOTFOUND for s3cret") });
  });

  await assert.rejects(
    callApi("https://api.example.com/things", { token: "s3cret" }),
    (e) =>
      e instanceof Refused &&
      e.message === "Could not reach api.example.com: getaddrinfo ENOTFOUND for ***. Check the network connection.",
  );
});

test("callApi turns a timeout into a refusal that says how long it waited", async (t) => {
  t.mock.method(globalThis, "fetch", async () => {
    throw new DOMException("The operation was aborted due to timeout", "TimeoutError");
  });

  await assert.rejects(
    callApi("https://api.example.com/things"),
    isRefusal(new RegExp(`^api\\.example\\.com did not answer within ${API_TIMEOUT_MS / 1000} seconds`)),
  );
});

/* ------------------------------------------------------------------ github() */

test("github sends the token and the pinned headers, builds the query, and returns the answer", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { ok: 1 }, { "x-thing": "1" }));

  const answer = await github("GET", "/repos/o/r/things", {
    query: { page: 2, per_page: 30, sha: "main", empty: "", missing: undefined, none: null },
  });

  assert.equal(answer.status, 200);
  assert.deepEqual(answer.body, { ok: 1 });
  assert.equal(answer.headers["x-thing"], "1");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/things?page=2&per_page=30&sha=main");
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].headers.Authorization, `Bearer ${TOKEN}`);
  assert.equal(calls[0].headers.Accept, "application/vnd.github+json");
  assert.equal(calls[0].headers["X-GitHub-Api-Version"], "2022-11-28");
  assert.equal(calls[0].headers["User-Agent"], `lxagents-mcp-github/${VERSION}`);
  assert.equal(calls[0].headers["Content-Type"], undefined);
});

test("github sends a JSON body with its content type, and a different Accept when asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 1 }));

  await github("POST", "/repos/o/r/things", { json: { a: 1 }, accept: "application/vnd.github.diff" });

  assert.deepEqual(calls[0].body, { a: 1 });
  assert.equal(calls[0].headers["Content-Type"], "application/json");
  assert.equal(calls[0].headers.Accept, "application/vnd.github.diff");
});

test("github with no token refuses before it sends anything", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(github("GET", "/user"), isRefusal(/^GITHUB_API_KEY is not set\./));
  assert.equal(calls.length, 0);
});

test("github refuses a path that could change which endpoint is called", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  for (const path of ["user", "/user?x=1", "/user#frag"]) {
    await assert.rejects(github("GET", path), (e) => e instanceof Error && !(e instanceof Refused), path);
  }
  assert.equal(calls.length, 0);
});

test("github returns a status listed in allow instead of refusing", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }));

  const answer = await github("GET", "/repos/o/r/git/ref/tags/v9", { allow: [404] });

  assert.equal(answer.status, 404);
});

test("401 says the token was rejected", async (t) => {
  withToken(t);
  mockApi(t, reply(401, { message: "Bad credentials" }));

  await assert.rejects(
    github("GET", "/user", { what: "read the user" }),
    isRefusal(/^GitHub rejected the token \(401\)\. Check that GITHUB_API_KEY holds a valid token/),
  );
});

test("403 says what permission is missing, with GitHub's own hint when it gives one", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { message: "Resource not accessible" }, { "x-accepted-github-permissions": "contents=write" }));

  await assert.rejects(
    github("PUT", "/repos/o/r/contents/a", { what: "write the file 'a' in o/r" }),
    isRefusal(
      /^GitHub refused to write the file 'a' in o\/r \(403\): Resource not accessible\. .*scope.*accepts: contents=write\)\.$/,
    ),
  );
});

test("a 403 that is a rate limit says when to try again", async (t) => {
  withToken(t);
  const reset = String(Math.floor(Date.now() / 1000) + 600);
  mockApi(t, reply(403, { message: "API rate limit exceeded" }, { "x-ratelimit-remaining": "0", "x-ratelimit-reset": reset }));

  await assert.rejects(
    github("GET", "/user"),
    isRefusal(/^GitHub is rate limiting this token \(403\): API rate limit exceeded\. The limit resets at .*in about (9|10|11) minutes\.$/),
  );
});

test("a 429 with retry-after says how long to wait", async (t) => {
  withToken(t);
  mockApi(t, reply(429, { message: "Slow down" }, { "retry-after": "30" }));

  await assert.rejects(github("GET", "/user"), isRefusal(/\(429\): Slow down\. Wait 30 seconds and try again\.$/));
});

test("404 says to check the spelling and that a private repository answers 404", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }));

  await assert.rejects(
    github("GET", "/repos/o/r", { what: "read the repository o/r" }),
    isRefusal(/^GitHub could not find what it needs to read the repository o\/r \(404\): Not Found\. .*private repository the token cannot see\.$/),
  );
});

test("409 and 422 carry GitHub's reasons", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(409, { message: "Merge conflict" }),
    reply(422, { message: "Validation Failed", errors: [{ message: "name is taken" }, { code: "invalid" }, "plain"] }),
  );

  await assert.rejects(github("POST", "/x", { what: "merge" }), isRefusal(/could not merge \(409\): Merge conflict\. .*read it again/));
  await assert.rejects(
    github("POST", "/x", { what: "create it" }),
    isRefusal(/^GitHub could not create it \(422\): Validation Failed \(name is taken; invalid; plain\)\.$/),
  );
});

test("a 5xx says GitHub failed, and any other status is reported with its number", async (t) => {
  withToken(t);
  mockApi(t, reply(503, { message: "Unavailable" }), reply(418, { message: "Teapot" }));

  await assert.rejects(github("GET", "/x", { what: "read x" }), isRefusal(/^GitHub failed to read x \(503\): Unavailable\. Try again shortly\.$/));
  await assert.rejects(github("GET", "/x", { what: "read x" }), isRefusal(/^GitHub answered 418: Teapot when asked to read x\.$/));
});

test("a tool's own wording wins, and undefined falls through to the general wording", async (t) => {
  withToken(t);
  mockApi(t, reply(422, { message: "bad" }), reply(404, { message: "gone" }));
  const explain = (status, detail) => (status === 422 ? `custom ${status}${detail}` : undefined);

  await assert.rejects(github("POST", "/x", { explain }), isRefusal(/^custom 422: bad$/));
  await assert.rejects(github("GET", "/x", { explain, what: "find x" }), isRefusal(/^GitHub could not find what it needs to find x \(404\)/));
});

test("a failure never carries the token, even when GitHub echoes it", async (t) => {
  withToken(t);
  mockApi(t, reply(500, { message: `bad token ${TOKEN}` }));

  await assert.rejects(
    github("GET", "/x"),
    (e) => e instanceof Refused && !e.message.includes(TOKEN) && e.message.includes("***"),
  );
});

test("apiDetail reads GitHub's message, its error list, and plain text", () => {
  assert.equal(apiDetail({ message: "m" }), ": m");
  assert.equal(apiDetail({ message: "m", errors: [{ message: "a" }, { code: "b" }] }), ": m (a; b)");
  assert.equal(apiDetail("plain"), ": plain");
  assert.equal(apiDetail(""), "");
  assert.equal(apiDetail(null), "");
  assert.equal(apiDetail({ other: 1 }), "");
});

/* ------------------------------------------------------------------ addressing */

test("parseRepo reads owner/name and builds the API prefix", () => {
  assert.deepEqual(parseRepo(" LXAgents-MCP/github "), {
    owner: "LXAgents-MCP",
    name: "github",
    full: "LXAgents-MCP/github",
    path: "/repos/LXAgents-MCP/github",
  });
  assert.equal(parseRepo("a/b.c_d-e").name, "b.c_d-e");
  assert.equal(parseRepo("user_shortcode/x").owner, "user_shortcode");
});

test("parseRepo refuses anything that is not exactly owner/name", () => {
  for (const value of ["", "  ", "solo", "a/b/c", "a/", "/b", "a/..", "a/.", "../b", "a/b c", "-a/b", "a/b?x=1", "a/b#c", "a b/c"]) {
    assert.throws(() => parseRepo(value), (e) => e instanceof Refused && /`repo`/.test(e.message), JSON.stringify(value));
  }
  assert.throws(() => parseRepo(undefined), isRefusal(/`repo` is required/));
  assert.throws(() => parseRepo(5), isRefusal(/`repo` must be a string, got number/));
  assert.throws(() => parseRepo("nope", "target"), isRefusal(/`target` 'nope' is not a repository\. Write it as 'owner\/name'/));
  assert.throws(() => parseRepo("a".repeat(40) + "/b"), isRefusal(/is not a repository/));
});

test("refName accepts what git accepts", () => {
  for (const value of ["main", "feature/login", "v1.2.3", "HEAD", "0123456789abcdef0123456789abcdef01234567", "release/1.0.0", "ünï"]) {
    assert.equal(refName(value, "ref"), value);
  }
  assert.equal(refName("  main ", "ref"), "main");
});

test("refName refuses what could change the path or that git itself refuses", () => {
  const bad = ["", "a..b", "a b", "-x", "a/", "/a", "a//b", ".hidden", "a/.hidden", "x.lock", "a/x.lock", "a~1", "a^", "a:b", "a?b", "a*b", "a[b", "a\\b", "a@{b", "end.", "a\nb", "a".repeat(256)];
  for (const value of bad) {
    assert.throws(() => refName(value, "ref"), (e) => e instanceof Refused, JSON.stringify(value));
  }
  assert.throws(() => refName("a b", "branch"), isRefusal(/^`branch` 'a b' is not a valid branch, tag or commit name\.$/));
});

test("refPath keeps the slashes and encodes everything else", () => {
  assert.equal(refPath("feature/login"), "feature/login");
  assert.equal(refPath("a#b/c d"), "a%23b/c%20d");
  assert.equal(refPath("100%"), "100%25");
});

test("filePath cleans a path and encodes each segment", () => {
  assert.deepEqual(filePath("/src/a b.js/", "path"), { path: "src/a b.js", encoded: "src/a%20b.js" });
  assert.deepEqual(filePath("README.md", "path"), { path: "README.md", encoded: "README.md" });
  assert.deepEqual(filePath("dir/ünï#.txt", "path"), { path: "dir/ünï#.txt", encoded: "dir/%C3%BCn%C3%AF%23.txt" });
});

test("filePath treats the root as valid only when allowed, and refuses traversal", () => {
  assert.deepEqual(filePath(undefined, "path", { allowEmpty: true }), { path: "", encoded: "" });
  assert.deepEqual(filePath("/", "path", { allowEmpty: true }), { path: "", encoded: "" });
  assert.throws(() => filePath("", "path"), isRefusal(/`path` is required/));
  for (const value of ["../x", "a/../b", "a/./b", "a//b", "..", "a\u0000b"]) {
    assert.throws(() => filePath(value, "path"), isRefusal(/is not a path inside the repository/), JSON.stringify(value));
  }
  assert.throws(() => filePath(5, "path"), isRefusal(/`path` must be a string/));
});

/* ------------------------------------------------------------------ paging */

test("workflowName reads an id or a file name and drops a leading workflows path", () => {
  assert.deepEqual(workflowName("ci.yml"), { name: "ci.yml", encoded: "ci.yml" });
  assert.deepEqual(workflowName(" 161335 "), { name: "161335", encoded: "161335" });
  assert.deepEqual(workflowName(".github/workflows/release.yaml"), { name: "release.yaml", encoded: "release.yaml" });
  assert.deepEqual(workflowName("build_and-test.v2.yml", "w"), { name: "build_and-test.v2.yml", encoded: "build_and-test.v2.yml" });
});

test("workflowName refuses what is not one name, and cannot be steered at another endpoint", () => {
  for (const value of ["a/b.yml", "../x.yml", "..", ".hidden.yml", "a b.yml", "a?x=1", "a#b", "ci.yml/dispatches", "x".repeat(256)]) {
    assert.throws(() => workflowName(value), isRefusal(/is not a workflow\. Give its numeric id, or the name of its file/), JSON.stringify(value));
  }
  assert.throws(() => workflowName(undefined), isRefusal(/`workflow` is required/));
  assert.throws(() => workflowName("  "), isRefusal(/`workflow` is required/));
  assert.throws(() => workflowName(12), isRefusal(/`workflow` must be a string/));
});

test("pageQuery defaults to the first page of thirty and bounds what it is given", () => {
  assert.deepEqual(pageQuery({}), { page: 1, per_page: 30 });
  assert.deepEqual(pageQuery({ page: 3, per_page: 100 }), { page: 3, per_page: 100 });
  assert.throws(() => pageQuery({ page: 0 }), isRefusal(/`page` must be between 1 and/));
  assert.throws(() => pageQuery({ per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100, got 101\./));
  assert.throws(() => pageQuery({ page: "2" }), isRefusal(/`page` must be a whole number/));
});

test("paging says whether GitHub has another page", () => {
  const query = { page: 2, per_page: 30 };

  assert.deepEqual(paging(query, { link: '<https://api.github.com/x?page=3>; rel="next", <y>; rel="last"' }), {
    page: 2,
    per_page: 30,
    has_more: true,
  });
  assert.equal(paging(query, { link: '<https://api.github.com/x?page=1>; rel="prev"' }).has_more, false);
  assert.equal(paging(query, {}).has_more, false);
});
