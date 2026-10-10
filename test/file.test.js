import assert from "node:assert/strict";
import test from "node:test";
import { MAX_FILE_BYTES, MAX_LIST_ENTRIES, MAX_WRITE_BYTES } from "../src/config.js";
import { blobSha, contentsPath, encodeContent } from "../src/contents.js";
import { TOOL as create, handle as fileCreate } from "../src/tools/file-create.js";
import { TOOL as del, handle as fileDelete } from "../src/tools/file-delete.js";
import { TOOL as dir, handle as directoryList } from "../src/tools/directory-list.js";
import { TOOL as read, handle as fileRead } from "../src/tools/file-read.js";
import { TOOL as update, handle as fileUpdate } from "../src/tools/file-update.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const b64 = (text) => Buffer.from(text).toString("base64");
const REPO = { full: "o/r", path: "/repos/o/r" };

/** A file as the contents API returns it: base64 wrapped across lines. */
const fileBody = (text, extra = {}) => ({
  type: "file",
  encoding: "base64",
  name: "a.txt",
  path: "a.txt",
  sha: "blob1",
  size: Buffer.byteLength(text),
  content: `${b64(text)}\n`,
  ...extra,
});

/** The answer to a write: the new blob and the commit that made it. */
const written = (commit = "c0ffee0000000000000000000000000000000000", blob = "blob2") => ({
  content: { sha: blob },
  commit: { sha: commit },
});

/* ------------------------------------------------------------------ the shared helpers */

test("contentsPath addresses the root and a path", () => {
  assert.equal(contentsPath(REPO, { encoded: "" }), "/repos/o/r/contents");
  assert.equal(contentsPath(REPO, { encoded: "src/a%20b.js" }), "/repos/o/r/contents/src/a%20b.js");
});

test("encodeContent is base64 of the UTF-8 bytes, and refuses a write over the limit", () => {
  assert.equal(encodeContent("héllo"), Buffer.from("héllo", "utf8").toString("base64"));
  assert.equal(encodeContent(""), "");
  assert.throws(() => encodeContent("x".repeat(MAX_WRITE_BYTES + 1)), isRefusal(/over the 1000000-byte limit for one write/));
  assert.doesNotThrow(() => encodeContent("x".repeat(MAX_WRITE_BYTES)));
});

test("blobSha reads the sha of a file on a branch, and says what to do when there is none", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, fileBody("x", { sha: "abc" })), reply(404, { message: "Not Found" }), reply(200, [{}]), reply(200, { type: "symlink" }));
  const file = { path: "a.txt", encoded: "a.txt" };

  assert.equal(await blobSha(REPO, file, "dev", "Use file_create."), "abc");
  assert.equal(new URL(calls[0].url).searchParams.get("ref"), "dev");
  await assert.rejects(blobSha(REPO, file, "dev", "Use file_create."), isRefusal(/^There is no file 'a\.txt' in o\/r on dev\. Use file_create\.$/));
  await assert.rejects(blobSha(REPO, file, undefined, "x"), isRefusal(/is a directory in o\/r, not a file/));
  await assert.rejects(blobSha(REPO, file, undefined, "x"), isRefusal(/is not a regular file/));
});

/* ------------------------------------------------------------------ file_read */

test("file_read decodes the file and reports its sha and size", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, fileBody("héllo\nworld\n", { sha: "deadbeef" })));

  const { payload } = await fileRead({ repo: "o/r", path: "/src/a b.js", ref: "dev" });

  assert.equal(calls[0].method, "GET");
  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/repos/o/r/contents/src/a%20b.js");
  assert.equal(url.searchParams.get("ref"), "dev");
  assert.deepEqual(payload, {
    summary: "Read src/a b.js (13 bytes) from o/r at dev.",
    path: "src/a b.js",
    sha: "deadbeef",
    size: 13,
    text: "héllo\nworld\n",
    truncated: false,
  });
  expectOutput(read, payload);
});

test("file_read sends no ref by default, and reads an empty file", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, fileBody("", { size: 0 })));

  const { payload } = await fileRead({ repo: "o/r", path: "empty.txt" });

  assert.equal(new URL(calls[0].url).searchParams.has("ref"), false);
  assert.equal(payload.text, "");
  assert.equal(payload.summary, "Read empty.txt (0 bytes) from o/r.");
  expectOutput(read, payload);
});

test("file_read falls back to Windows-1252, and cuts a long file at the limit", async (t) => {
  withToken(t);
  const latin = Buffer.from([0x63, 0x61, 0x66, 0xe9]);
  const long = "a".repeat(MAX_FILE_BYTES + 50);
  mockApi(t, reply(200, fileBody("", { content: latin.toString("base64"), size: 4 })), reply(200, fileBody(long)));

  const one = await fileRead({ repo: "o/r", path: "latin.txt" });
  assert.equal(one.payload.text, "café");

  const two = await fileRead({ repo: "o/r", path: "long.txt" });
  assert.equal(two.payload.truncated, true);
  assert.equal(two.payload.text.length, MAX_FILE_BYTES);
  assert.equal(two.payload.size, MAX_FILE_BYTES + 50);
  assert.match(two.payload.summary, /, cut at 200000 bytes\.$/);
  expectOutput(read, two.payload);
});

test("file_read refuses a binary file, a directory, a link, a submodule and a body it does not know", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, fileBody("", { content: Buffer.from([1, 0, 2]).toString("base64"), size: 3 })),
    reply(200, [{ name: "x", type: "file" }]),
    reply(200, { type: "symlink", target: "../real.txt" }),
    reply(200, { type: "submodule", submodule_git_url: "https://example.com/m.git" }),
    reply(200, { type: "file", encoding: "none", content: "" }),
  );

  await assert.rejects(fileRead({ repo: "o/r", path: "bin" }), isRefusal(/is a binary file \(3 bytes\); this tool reads text/));
  await assert.rejects(fileRead({ repo: "o/r", path: "dir" }), isRefusal(/is a directory in o\/r\. Use directory_list/));
  await assert.rejects(fileRead({ repo: "o/r", path: "ln" }), isRefusal(/is a symbolic link to '\.\.\/real\.txt'\. Read that path instead/));
  await assert.rejects(fileRead({ repo: "o/r", path: "sub" }), isRefusal(/is a submodule of https:\/\/example\.com\/m\.git/));
  await assert.rejects(fileRead({ repo: "o/r", path: "big" }), isRefusal(/the body was not a file this server recognizes/));
});

test("file_read explains a file over 1 MB and a missing file, and refuses a bad path before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(403, { message: "This API returns blobs up to 1 MB in size.", errors: [{ code: "too_large" }] }),
    reply(404, { message: "Not Found" }),
  );

  await assert.rejects(fileRead({ repo: "o/r", path: "big.bin" }), isRefusal(/is over the 1 MB GitHub returns through this API \(403\).*Fetch it with git instead/));
  await assert.rejects(fileRead({ repo: "o/r", path: "nope.txt" }), isRefusal(/could not find what it needs to read 'nope\.txt' in o\/r \(404\)/));
  assert.equal(calls.length, 2);

  await assert.rejects(fileRead({ repo: "o/r" }), isRefusal(/`path` is required/));
  await assert.rejects(fileRead({ repo: "o/r", path: "../etc/passwd" }), isRefusal(/is not a path inside the repository/));
  await assert.rejects(fileRead({ repo: "o/r", path: "a", ref: "x y" }), isRefusal(/`ref` 'x y' is not a valid/));
  assert.equal(calls.length, 2);
});

/* ------------------------------------------------------------------ directory_list */

test("directory_list lists the root and a subdirectory, with each entry's type and size", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [
      { name: "src", type: "dir" },
      { name: "README.md", type: "file", size: 120 },
    ]),
    reply(200, [{ name: "index.js", type: "file", size: 9 }]),
  );

  const root = await directoryList({ repo: "o/r" });
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/contents");
  assert.deepEqual(root.payload, {
    summary: "2 entries in the root of o/r.",
    path: "",
    items: [
      { name: "src", type: "dir", size: 0 },
      { name: "README.md", type: "file", size: 120 },
    ],
    count: 2,
    truncated: false,
  });
  expectOutput(dir, root.payload);

  const sub = await directoryList({ repo: "o/r", path: "/src/tools/", ref: "dev" });
  const url = new URL(calls[1].url);
  assert.equal(url.pathname, "/repos/o/r/contents/src/tools");
  assert.equal(url.searchParams.get("ref"), "dev");
  assert.equal(sub.payload.summary, "1 entry in 'src/tools' of o/r at dev.");
});

test("directory_list shows the first entries of a very large directory and says so", async (t) => {
  withToken(t);
  mockApi(t, reply(200, Array.from({ length: MAX_LIST_ENTRIES + 25 }, (_, i) => ({ name: `f${i}`, type: "file", size: 1 }))));

  const { payload } = await directoryList({ repo: "o/r", path: "big" });

  assert.equal(payload.items.length, MAX_LIST_ENTRIES);
  assert.equal(payload.count, MAX_LIST_ENTRIES + 25);
  assert.equal(payload.truncated, true);
  assert.match(payload.summary, /; showing the first 500\.$/);
  expectOutput(dir, payload);
});

test("directory_list refuses a file, reports a missing path, and refuses a bad path before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, fileBody("x")), reply(404, { message: "Not Found" }));

  await assert.rejects(directoryList({ repo: "o/r", path: "a.txt" }), isRefusal(/is a file in o\/r, not a directory\. Use file_read/));
  await assert.rejects(directoryList({ repo: "o/r", path: "nope" }), isRefusal(/could not find what it needs to list 'nope' of o\/r \(404\)/));
  await assert.rejects(directoryList({ repo: "o/r", path: "a/../b" }), isRefusal(/is not a path inside the repository/));
  assert.equal(calls.length, 2);
});

/* ------------------------------------------------------------------ file_create */

test("file_create commits the content as base64 and reports the blob and the commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, written("1234567000000000000000000000000000000000", "blobnew")));

  const { payload } = await fileCreate({ repo: "o/r", path: "docs/n.md", content: "# Notes\n", message: " docs: add notes ", branch: "dev" });

  assert.equal(calls[0].method, "PUT");
  assert.equal(calls[0].url, "https://api.github.com/repos/o/r/contents/docs/n.md");
  assert.deepEqual(calls[0].body, { message: "docs: add notes", content: b64("# Notes\n"), branch: "dev" });
  assert.deepEqual(payload, {
    summary: "Created docs/n.md in o/r on dev (commit 1234567).",
    path: "docs/n.md",
    sha: "blobnew",
    commit_sha: "1234567000000000000000000000000000000000",
    branch: "dev",
  });
  expectOutput(create, payload);
});

test("file_create sends no branch by default, and creates an empty file", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, written()));

  const { payload } = await fileCreate({ repo: "o/r", path: "empty", content: "", message: "add empty" });

  assert.deepEqual(calls[0].body, { message: "add empty", content: "" });
  assert.match(payload.summary, /^Created empty in o\/r on the default branch \(commit c0ffee0\)\.$/);
  assert.equal("branch" in payload, false);
});

test("file_create points at file_update when the file exists, and explains a moved branch", async (t) => {
  withToken(t);
  mockApi(t, reply(422, { message: 'Invalid request.\n\n"sha" wasn\'t supplied.' }), reply(409, { message: "is at abc but expected def" }));

  await assert.rejects(
    fileCreate({ repo: "o/r", path: "a.txt", content: "x", message: "m", branch: "dev" }),
    isRefusal(/The file already exists on dev; use file_update to change it\.$/),
  );
  await assert.rejects(fileCreate({ repo: "o/r", path: "a.txt", content: "x", message: "m" }), isRefusal(/\(409\).*The branch moved while the commit was made; try again\.$/));
});

test("file_create refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, written()));

  await assert.rejects(fileCreate({ repo: "o/r", content: "x", message: "m" }), isRefusal(/`path` is required/));
  await assert.rejects(fileCreate({ repo: "o/r", path: "a", message: "m" }), isRefusal(/`content` is required/));
  await assert.rejects(fileCreate({ repo: "o/r", path: "a", content: 5, message: "m" }), isRefusal(/`content` must be a string/));
  await assert.rejects(fileCreate({ repo: "o/r", path: "a", content: "x" }), isRefusal(/`message` is required/));
  await assert.rejects(fileCreate({ repo: "o/r", path: "../a", content: "x", message: "m" }), isRefusal(/is not a path inside the repository/));
  await assert.rejects(fileCreate({ repo: "o/r", path: "a", content: "x".repeat(MAX_WRITE_BYTES + 1), message: "m" }), isRefusal(/over the 1000000-byte limit/));
  assert.equal(calls.length, 0);
});

test("file_create refuses an answer with no commit", async (t) => {
  withToken(t);
  mockApi(t, reply(201, { content: { sha: "x" } }));

  await assert.rejects(fileCreate({ repo: "o/r", path: "a", content: "x", message: "m" }), isRefusal(/the body was not a commit this server recognizes/));
});

/* ------------------------------------------------------------------ file_update */

test("file_update reads the current sha, then replaces the file", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, fileBody("old", { sha: "oldsha" })), reply(200, written("abcdef1000000000000000000000000000000000", "newsha")));

  const { payload } = await fileUpdate({ repo: "o/r", path: "a.txt", content: "new", message: "update a", branch: "dev" });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].method, "GET");
  assert.equal(new URL(calls[0].url).searchParams.get("ref"), "dev");
  assert.equal(calls[1].method, "PUT");
  assert.deepEqual(calls[1].body, { message: "update a", content: b64("new"), sha: "oldsha", branch: "dev" });
  assert.deepEqual(payload, {
    summary: "Updated a.txt in o/r on dev (commit abcdef1).",
    path: "a.txt",
    sha: "newsha",
    commit_sha: "abcdef1000000000000000000000000000000000",
    branch: "dev",
  });
  expectOutput(update, payload);
});

test("file_update uses a sha it is given and makes a single request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, written()));

  await fileUpdate({ repo: "o/r", path: "a.txt", content: "new", message: "m", sha: " mine " });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].body.sha, "mine");
  assert.equal("branch" in calls[0].body, false);
});

test("file_update points at file_create for a missing file, and says a changed file must be read again", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "Not Found" }), reply(409, { message: "a.txt does not match abc" }), reply(422, { message: "sha wasn't supplied" }));

  await assert.rejects(
    fileUpdate({ repo: "o/r", path: "a.txt", content: "x", message: "m" }),
    isRefusal(/^There is no file 'a\.txt' in o\/r\. Use file_create to make it\.$/),
  );
  await assert.rejects(
    fileUpdate({ repo: "o/r", path: "a.txt", content: "x", message: "m", sha: "stale" }),
    isRefusal(/\(409\): a\.txt does not match abc\. The file changed since its sha was read; read it again with file_read/),
  );
  await assert.rejects(fileUpdate({ repo: "o/r", path: "a.txt", content: "x", message: "m", sha: "stale" }), isRefusal(/\(422\).*read it again/));
});

test("file_update refuses a directory at the path and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [{ name: "x" }]));

  await assert.rejects(fileUpdate({ repo: "o/r", path: "dir", content: "x", message: "m" }), isRefusal(/is a directory in o\/r, not a file/));
  assert.equal(calls.length, 1);
  await assert.rejects(fileUpdate({ repo: "o/r", path: "a", message: "m" }), isRefusal(/`content` is required/));
  await assert.rejects(fileUpdate({ repo: "o/r", path: "a", content: "x" }), isRefusal(/`message` is required/));
  await assert.rejects(fileUpdate({ repo: "o/r", path: "a", content: "x", message: "m", sha: 5 }), isRefusal(/`sha` must be a string/));
  assert.equal(calls.length, 1);
});

/* ------------------------------------------------------------------ file_delete */

test("file_delete reads the current sha, then deletes with it", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, fileBody("x", { sha: "gone1" })), reply(200, { content: null, commit: { sha: "9876543000000000000000000000000000000000" } }));

  const { payload } = await fileDelete({ repo: "o/r", path: "old.txt", message: "remove old", branch: "dev" });

  assert.equal(calls[1].method, "DELETE");
  assert.equal(calls[1].url, "https://api.github.com/repos/o/r/contents/old.txt");
  assert.deepEqual(calls[1].body, { message: "remove old", sha: "gone1", branch: "dev" });
  assert.deepEqual(payload, {
    summary: "Deleted old.txt from o/r on dev (commit 9876543).",
    deleted: true,
    path: "old.txt",
    commit_sha: "9876543000000000000000000000000000000000",
  });
  expectOutput(del, payload);
});

test("file_delete uses a sha it is given, and says there is nothing to delete for a missing file", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { commit: { sha: "c" } }), reply(404, { message: "Not Found" }), reply(409, { message: "mismatch" }));

  await fileDelete({ repo: "o/r", path: "a", message: "m", sha: "s1" });
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].body, { message: "m", sha: "s1" });

  await assert.rejects(fileDelete({ repo: "o/r", path: "a", message: "m" }), isRefusal(/^There is no file 'a' in o\/r\. There is nothing to delete\.$/));
  await assert.rejects(fileDelete({ repo: "o/r", path: "a", message: "m", sha: "old" }), isRefusal(/The file changed since its sha was read; read it again with file_read before deleting/));
});

test("file_delete refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(fileDelete({ repo: "o/r", message: "m" }), isRefusal(/`path` is required/));
  await assert.rejects(fileDelete({ repo: "o/r", path: "a" }), isRefusal(/`message` is required/));
  await assert.rejects(fileDelete({ repo: "o/r", path: "./a", message: "m" }), isRefusal(/is not a path inside the repository/));
  assert.equal(calls.length, 0);
});
