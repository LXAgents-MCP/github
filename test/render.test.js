import assert from "node:assert/strict";
import test from "node:test";
import { MAX_OUTPUT } from "../src/config.js";
import { describe, fence, renderPayload } from "../src/render.js";
import { decodeText, firstLine, head, looksBinary, short, tail } from "../src/text.js";

test("a flat payload is a summary and one bullet per scalar, in order", () => {
  assert.equal(
    renderPayload("Created it.", { name: "demo", count: 3, draft: false }),
    "**Created it.**\n\n- name: demo\n- count: 3\n- draft: false",
  );
});

test("absent fields, empty lists and nested objects are left out of the text", () => {
  assert.equal(
    renderPayload("S", { a: undefined, b: null, c: [], d: { nested: 1 }, e: "kept" }),
    "**S**\n\n- e: kept",
  );
});

test("a list of scalars is one bullet, joined with commas", () => {
  assert.equal(renderPayload("S", { labels: ["bug", "ui"] }), "**S**\n\n- labels: bug, ui");
});

test("a list of rows is a heading and one bullet per row, scalars only", () => {
  assert.equal(
    renderPayload("2 branches.", {
      items: [
        { name: "main", sha: "abc1234", protected: true, nested: { x: 1 } },
        { name: "dev", sha: "def5678", protected: false, tags: ["a", "b"] },
      ],
    }),
    "**2 branches.**\n\n**items**\n- name: main · sha: abc1234 · protected: true\n- name: dev · sha: def5678 · protected: false · tags: a, b",
  );
});

test("a multi-line or very long value stays on one line in a row", () => {
  const long = "x".repeat(300);
  const text = renderPayload("S", { items: [{ body: "line one\nline two", long }] });
  assert.match(text, /- body: line one line two · long: x{200}…$/);
});

test("text is fenced as a block after the bullets", () => {
  assert.equal(
    renderPayload("Read it.", { path: "a.txt", text: "hello\n" }),
    "**Read it.**\n\n- path: a.txt\n\n```text\nhello\n```",
  );
});

test("an empty text field adds no block", () => {
  assert.equal(renderPayload("S", { text: "" }), "**S**");
});

test("a fence is longer than any run of backticks inside the block", () => {
  assert.equal(fence("a ``` b"), "````text\na ``` b\n````");
  assert.equal(fence("plain"), "```text\nplain\n```");
});

test("describe renders a row on one line and a scalar as itself", () => {
  assert.equal(describe({ a: 1, b: "x", c: undefined, d: null, e: "" }), "a: 1 · b: x");
  assert.equal(describe("solo"), "solo");
  assert.equal(describe(null), "");
});

test("the rendered text is cut at the output ceiling", () => {
  const text = renderPayload("S", { text: "y".repeat(MAX_OUTPUT * 2) });
  assert.ok(text.length <= MAX_OUTPUT + 20);
  assert.match(text, /\.\.\. \(truncated\)$/);
});

test("short, firstLine, head and tail", () => {
  assert.equal(short("0123456789"), "0123456");
  assert.equal(short(undefined), "");
  assert.equal(firstLine("one\ntwo"), "one");
  assert.equal(firstLine(undefined), "");
  assert.deepEqual(head("abcdef", 3), ["abc\n... (truncated)", true]);
  assert.deepEqual(head("abc", 3), ["abc", false]);
  assert.deepEqual(tail("abcdef", 3), ["(truncated) ...\ndef", true]);
  assert.deepEqual(tail("abc", 3), ["abc", false]);
});

test("decodeText reads UTF-8, falls back to Windows-1252, and keeps the byte order mark", () => {
  assert.deepEqual(decodeText(Buffer.from("héllo", "utf8")), { text: "héllo", truncated: false });
  assert.equal(decodeText(Buffer.from([0x63, 0x61, 0x66, 0xe9])).text, "café");
  assert.equal(decodeText(Buffer.from([0xef, 0xbb, 0xbf, 0x61])).text, "\uFEFFa");
});

test("decodeText cuts at a byte count and drops a character the cut split", () => {
  const bytes = Buffer.from("aé", "utf8"); // 61 c3 a9
  assert.deepEqual(decodeText(bytes, { max: 2 }), { text: "a", truncated: true });
  assert.deepEqual(decodeText(bytes, { max: 3 }), { text: "aé", truncated: false });
  assert.deepEqual(decodeText(Buffer.from("abcdef"), { max: 3 }), { text: "abc", truncated: true });
});

test("looksBinary is true for a NUL byte early on and false for text", () => {
  assert.equal(looksBinary(Buffer.from([0x68, 0x00, 0x69])), true);
  assert.equal(looksBinary(Buffer.from("plain text")), false);
  assert.equal(looksBinary(Buffer.concat([Buffer.alloc(9000, 0x61), Buffer.from([0])])), false);
});
