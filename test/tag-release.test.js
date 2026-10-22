import assert from "node:assert/strict";
import test from "node:test";
import { findReleaseByTag } from "../src/services/releases.js";
import { TOOL as create, handle as releaseCreate } from "../src/controllers/release-create.js";
import { TOOL as del, handle as releaseDelete } from "../src/controllers/release-delete.js";
import { TOOL as list, handle as releaseList } from "../src/controllers/release-list.js";
import { TOOL as read, handle as releaseRead } from "../src/controllers/release-read.js";
import { TOOL as check, handle as tagCheck } from "../src/controllers/tag-check.js";
import { TOOL as tagDel, handle as tagDelete } from "../src/controllers/tag-delete.js";
import { TOOL as getAll, handle as tagGetAll } from "../src/controllers/tag-get-all.js";
import { TOOL as get, handle as tagGet } from "../src/controllers/tag-get.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const COMMIT = "0123456789abcdef0123456789abcdef01234567";
const TAGOBJ = "fedcba9876543210fedcba9876543210fedcba98";
const REPO = { full: "o/r", path: "/repos/o/r" };

/** A release as GitHub returns it. */
const release = (overrides = {}) => ({
  id: 42,
  tag_name: "v1.0.0",
  name: "First release",
  draft: false,
  prerelease: false,
  html_url: "https://github.com/o/r/releases/tag/v1.0.0",
  published_at: "2026-10-01T00:00:00Z",
  body: "Notes",
  assets: [{ name: "app.zip", size: 1024, download_count: 7 }],
  ...overrides,
});

/* ------------------------------------------------------------------ tag_check */

test("tag_check says a tag exists, whether it is annotated, and what it points at", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { ref: "refs/tags/v1", object: { type: "commit", sha: COMMIT } }),
    reply(200, { ref: "refs/tags/v2", object: { type: "tag", sha: TAGOBJ } }),
  );

  const light = await tagCheck({ repo: "o/r", tag: "v1" });
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/git/ref/tags/v1");
  assert.deepEqual(light.payload, {
    summary: "Tag v1 exists in o/r (lightweight) and points at 0123456.",
    exists: true,
    tag: "v1",
    sha: COMMIT,
    annotated: false,
  });
  expectOutput(check, light.payload);

  const annotated = await tagCheck({ repo: "o/r", tag: "v2" });
  assert.equal(annotated.payload.annotated, true);
  assert.match(annotated.payload.summary, /\(annotated\) and points at fedcba9\.$/);
});

test("tag_check treats a missing tag as an answer, not an error", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }));

  const { payload, ok } = await tagCheck({ repo: "o/r", tag: "v9" });

  assert.equal(ok, true);
  assert.deepEqual(payload, { summary: "Tag v9 does not exist in o/r.", exists: false, tag: "v9" });
  expectOutput(check, payload);
});

test("tag_check still refuses a real failure and a bad argument", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(403, { message: "Resource not accessible" }));

  await assert.rejects(tagCheck({ repo: "o/r", tag: "v1" }), isRefusal(/GitHub refused to look for the tag 'v1' in o\/r \(403\)/));
  await assert.rejects(tagCheck({ repo: "o/r" }), isRefusal(/`tag` is required/));
  await assert.rejects(tagCheck({ repo: "o/r", tag: "a..b" }), isRefusal(/`tag` 'a\.\.b' is not a valid/));
  assert.equal(calls.length, 1);
});

/* ------------------------------------------------------------------ tag_get */

test("tag_get reads a lightweight tag in one request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { object: { type: "commit", sha: COMMIT } }));

  const { payload } = await tagGet({ repo: "o/r", tag: "v1" });

  assert.equal(calls.length, 1);
  assert.deepEqual(payload, {
    summary: "Tag v1 is a lightweight tag pointing at 0123456.",
    tag: "v1",
    sha: COMMIT,
    commit_sha: COMMIT,
    target_type: "commit",
    annotated: false,
  });
  expectOutput(get, payload);
});

test("tag_get follows an annotated tag to its commit, message and tagger", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { object: { type: "tag", sha: TAGOBJ } }),
    reply(200, { message: "Release 2\n", tagger: { name: "Ada", date: "2026-10-01T00:00:00Z" }, object: { type: "commit", sha: COMMIT } }),
  );

  const { payload } = await tagGet({ repo: "o/r", tag: "release/2.0" });

  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/git/ref/tags/release/2.0");
  assert.equal(calls[1].url, `https://api.github.com/repos/o/r/git/tags/${TAGOBJ}`);
  assert.deepEqual(payload, {
    summary: "Tag release/2.0 is annotated and points at 0123456.",
    tag: "release/2.0",
    sha: TAGOBJ,
    commit_sha: COMMIT,
    target_type: "commit",
    annotated: true,
    message: "Release 2\n",
    tagger: "Ada",
    date: "2026-10-01T00:00:00Z",
  });
  expectOutput(get, payload);
});

test("tag_get says there is no such tag, and refuses a body it does not know", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }), reply(200, { unexpected: true }), reply(200, { object: { type: "tag", sha: TAGOBJ } }), reply(200, { nope: 1 }));

  await assert.rejects(tagGet({ repo: "o/r", tag: "v9" }), isRefusal(/^There is no tag 'v9' in o\/r\. List them with tag_get_all\.$/));
  await assert.rejects(tagGet({ repo: "o/r", tag: "v1" }), isRefusal(/the body was not a tag reference this server recognizes/));
  await assert.rejects(tagGet({ repo: "o/r", tag: "v2" }), isRefusal(/the body was not an annotated tag this server recognizes/));
});

/* ------------------------------------------------------------------ tag_get_all */

test("tag_get_all lists the tags with their commits, and pages", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [{ name: "v2", commit: { sha: COMMIT } }, { name: "v1", commit: { sha: TAGOBJ } }], { link: '<x>; rel="next"' }),
    reply(200, []),
  );

  const { payload } = await tagGetAll({ repo: "o/r", per_page: 2 });
  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { page: "1", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 tags of o/r on page 1.",
    items: [{ name: "v2", sha: COMMIT }, { name: "v1", sha: TAGOBJ }],
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(getAll, payload);

  const empty = await tagGetAll({ repo: "o/r", page: 3 });
  assert.equal(empty.payload.summary, "o/r has no tags on page 3.");
});

test("tag_get_all refuses bad paging before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(tagGetAll({ repo: "o/r", per_page: 500 }), isRefusal(/`per_page` must be between 1 and 100/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ tag_delete */

test("tag_delete removes the tag reference", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  const { payload } = await tagDelete({ repo: "o/r", tag: "v1" });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/git/refs/tags/v1");
  assert.deepEqual(payload, { summary: "Deleted tag v1 from o/r.", deleted: true, tag: "v1" });
  expectOutput(tagDel, payload);
});

test("tag_delete explains a 422 as a tag that is not there, and refuses a bad tag before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(422, { message: "Reference does not exist" }));

  await assert.rejects(tagDelete({ repo: "o/r", tag: "v9" }), isRefusal(/^GitHub would not delete the tag 'v9' from o\/r \(422\): Reference does not exist\. It probably does not exist/));
  await assert.rejects(tagDelete({ repo: "o/r", tag: "x y" }), isRefusal(/is not a valid/));
  assert.equal(calls.length, 1);
});

/* ------------------------------------------------------------------ releases.js */

test("findReleaseByTag returns the published release, then searches the list for a draft", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, release()),
    reply(404, { message: "Not Found" }),
    reply(200, [release({ id: 1, tag_name: "other" }), release({ id: 2, tag_name: "v2", draft: true })]),
    reply(404, { message: "Not Found" }),
    reply(200, []),
  );

  assert.equal((await findReleaseByTag(REPO, "v1.0.0")).id, 42);
  assert.equal(calls.length, 1);

  const draft = await findReleaseByTag(REPO, "v2");
  assert.equal(draft.id, 2);
  assert.equal(draft.draft, true);
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/releases/tags/v2");
  assert.deepEqual(Object.fromEntries(new URL(calls[2].url).searchParams), { per_page: "100", page: "1" });

  await assert.rejects(findReleaseByTag(REPO, "ghost"), isRefusal(/^There is no release for the tag 'ghost' in o\/r\. Check the tag name, or list them with release_list\.$/));
});

test("findReleaseByTag searches up to three pages of a long list", async (t) => {
  withToken(t);
  const full = (n) => Array.from({ length: 100 }, (_, i) => release({ id: n * 100 + i, tag_name: `t${n}-${i}` }));
  const { calls } = mockApi(t, reply(404, {}), reply(200, full(1)), reply(200, full(2)), reply(200, [release({ id: 9, tag_name: "wanted" })]));

  assert.equal((await findReleaseByTag(REPO, "wanted")).id, 9);
  assert.equal(calls.length, 4);
});

/* ------------------------------------------------------------------ release_create */

test("release_create sends what was given, creating the tag from the target", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, release({ draft: true })));

  const { payload } = await releaseCreate({
    repo: "o/r",
    tag: "v1.0.0",
    target: "main",
    name: " First release ",
    body: "Notes\n",
    draft: true,
    prerelease: false,
    generate_notes: true,
  });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/releases");
  assert.deepEqual(calls[0].body, {
    tag_name: "v1.0.0",
    target_commitish: "main",
    name: "First release",
    body: "Notes\n",
    draft: true,
    prerelease: false,
    generate_release_notes: true,
  });
  assert.equal(payload.summary, "Created draft release First release (tag v1.0.0) in o/r.");
  assert.equal(payload.id, 42);
  assert.deepEqual(payload.assets, [{ name: "app.zip", size: 1024, downloads: 7 }]);
  expectOutput(create, payload);
});

test("release_create sends only the tag by default", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, release({ name: null, body: null, assets: undefined })));

  const { payload } = await releaseCreate({ repo: "o/r", tag: "v1.0.0" });

  assert.deepEqual(calls[0].body, { tag_name: "v1.0.0" });
  assert.equal(payload.summary, "Created release v1.0.0 (tag v1.0.0) in o/r.");
  assert.deepEqual(payload.assets, []);
  assert.equal("name" in payload, false);
  assert.equal("body" in payload, false);
});

test("release_create says when a release for the tag already exists", async (t) => {
  withToken(t);
  mockApi(t, reply(422, { message: "Validation Failed", errors: [{ resource: "Release", code: "already_exists", field: "tag_name" }] }));

  await assert.rejects(releaseCreate({ repo: "o/r", tag: "v1.0.0" }), isRefusal(/A release for that tag already exists; read it with release_read, or use another tag\.$/));
});

test("release_create refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, release()));

  await assert.rejects(releaseCreate({ repo: "o/r" }), isRefusal(/`tag` is required/));
  await assert.rejects(releaseCreate({ repo: "o/r", tag: "a b" }), isRefusal(/`tag` 'a b' is not a valid/));
  await assert.rejects(releaseCreate({ repo: "o/r", tag: "v1", target: "x y" }), isRefusal(/`target` 'x y' is not a valid/));
  await assert.rejects(releaseCreate({ repo: "o/r", tag: "v1", draft: "yes" }), isRefusal(/`draft` must be true or false/));
  await assert.rejects(releaseCreate({ repo: "o/r", tag: "v1", body: 5 }), isRefusal(/`body` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ release_read */

test("release_read reads a release by tag", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, release()));

  const { payload } = await releaseRead({ repo: "o/r", tag: "v1.0.0" });

  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/releases/tags/v1.0.0");
  assert.equal(payload.summary, "o/r has release First release (tag v1.0.0).");
  assert.equal(payload.body, "Notes");
  assert.equal(payload.published_at, "2026-10-01T00:00:00Z");
  expectOutput(read, payload);
});

test("release_read reads the latest release when no tag is given, and says when there is none", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, release({ prerelease: false })), reply(404, { message: "Not Found" }));

  await releaseRead({ repo: "o/r" });
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/releases/latest");

  await assert.rejects(releaseRead({ repo: "o/r" }), isRefusal(/^o\/r has no published release yet\. List what exists with release_list\.$/));
});

test("release_read finds a draft by tag through the list", async (t) => {
  withToken(t);
  mockApi(t, reply(404, {}), reply(200, [release({ id: 7, tag_name: "v2", draft: true, name: "Next" })]));

  const { payload } = await releaseRead({ repo: "o/r", tag: "v2" });

  assert.equal(payload.draft, true);
  assert.equal(payload.summary, "o/r has draft release Next (tag v2).");
});

test("release_read refuses a bad tag before any request and a body that is not a release", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { nope: 1 }));

  await assert.rejects(releaseRead({ repo: "o/r", tag: "../x" }), isRefusal(/is not a valid/));
  assert.equal(calls.length, 0);
  await assert.rejects(releaseRead({ repo: "o/r", tag: "v1" }), isRefusal(/the body was not a release this server recognizes/));
});

/* ------------------------------------------------------------------ release_list */

test("release_list lists releases with their state, and pages", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [release(), release({ id: 41, tag_name: "v0.9.0", draft: true, name: null, published_at: null })], { link: '<x>; rel="next"' }),
  );

  const { payload } = await releaseList({ repo: "o/r", per_page: 2 });

  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { page: "1", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 releases of o/r on page 1.",
    items: [
      { id: 42, tag: "v1.0.0", name: "First release", draft: false, prerelease: false, published_at: "2026-10-01T00:00:00Z" },
      { id: 41, tag: "v0.9.0", draft: true, prerelease: false },
    ],
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("release_list reports an empty page", async (t) => {
  withToken(t);
  mockApi(t, reply(200, []));

  const { payload } = await releaseList({ repo: "o/r" });

  assert.equal(payload.summary, "o/r has no releases on page 1.");
});

/* ------------------------------------------------------------------ release_delete */

test("release_delete resolves a tag to its id and deletes, leaving the tag", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, release()), reply(204, undefined));

  const { payload } = await releaseDelete({ repo: "o/r", tag: "v1.0.0" });

  assert.equal(calls[1].method, "DELETE");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/releases/42");
  assert.deepEqual(payload, { summary: "Deleted the release for v1.0.0 of o/r. Its tag was kept.", deleted: true, id: 42, tag: "v1.0.0" });
  expectOutput(del, payload);
});

test("release_delete takes an id directly and makes a single request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  const { payload } = await releaseDelete({ repo: "o/r", release_id: 42 });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/releases/42");
  assert.equal(payload.summary, "Deleted the release 42 of o/r. Its tag was kept.");
  assert.equal("tag" in payload, false);
});

test("release_delete needs exactly one of tag and release_id, and a real id", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204, undefined));

  await assert.rejects(releaseDelete({ repo: "o/r" }), isRefusal(/^Give exactly one of `tag` and `release_id`\.$/));
  await assert.rejects(releaseDelete({ repo: "o/r", tag: "v1", release_id: 4 }), isRefusal(/Give exactly one of/));
  await assert.rejects(releaseDelete({ repo: "o/r", release_id: 0 }), isRefusal(/`release_id` must be between 1 and/));
  await assert.rejects(releaseDelete({ repo: "o/r", release_id: "4" }), isRefusal(/`release_id` must be a whole number/));
  assert.equal(calls.length, 0);
});

test("release_delete says when there is no release for the tag", async (t) => {
  withToken(t);
  mockApi(t, reply(404, {}), reply(200, []));

  await assert.rejects(releaseDelete({ repo: "o/r", tag: "ghost" }), isRefusal(/^There is no release for the tag 'ghost' in o\/r/));
});
