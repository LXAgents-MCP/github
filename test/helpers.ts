/*
 * Shared test support. Not a test file: `npm test` runs `test/*.test.ts` only, so this is
 * imported by the tests and never run on its own.
 */
import assert from "node:assert/strict";
import type { TestContext } from "node:test";
import { z } from "zod";
import type { ToolDefinition } from "../src/models/tool.js";
import { TOKEN_ENV } from "../src/config/config.js";
import { Refused } from "../src/utils/errors.js";

export const TOKEN = "ghp_test_token_1234";

/** Matches a refusal whose message matches `pattern`. */
export const isRefusal =
  (pattern: RegExp) =>
  (e: unknown): boolean =>
    e instanceof Refused && pattern.test(e.message);

/** Set the token variable for one test, or remove it when `value` is null, and restore it after. */
export function setToken(t: TestContext, value: string | null): void {
  const saved = process.env[TOKEN_ENV];
  if (value === null) delete process.env[TOKEN_ENV];
  else process.env[TOKEN_ENV] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[TOKEN_ENV];
    else process.env[TOKEN_ENV] = saved;
  });
}

/** Set any environment variable for one test, or remove it when `value` is null, and restore it after. */
export function setEnv(t: TestContext, name: string, value: string | null): void {
  const saved = process.env[name];
  if (value === null) delete process.env[name];
  else process.env[name] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[name];
    else process.env[name] = saved;
  });
}

export const withToken = (t: TestContext): void => setToken(t, TOKEN);
export const withoutToken = (t: TestContext): void => setToken(t, null);

/** A row of a list a tool returned, read in an assertion. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Row = Record<string, any>;

/** One canned answer for `mockApi`. */
export interface Answer {
  status: number;
  body?: unknown;
  headers?: Record<string, string>;
}

/** One request `mockApi` saw. `body` is the JSON it carried, decoded. */
export interface Call {
  url: string;
  method: string;
  headers: Record<string, string>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  body: any;
}

export const reply = (status: number, body?: unknown, headers: Record<string, string> = {}): Answer => ({
  status,
  body,
  headers,
});

/**
 * Replace `fetch` with a queue of answers, one per request in order, and record every request.
 * A request beyond the queue fails the test, so an unexpected call cannot go unnoticed.
 */
export function mockApi(t: TestContext, ...answers: Answer[]): { calls: Call[] } {
  const calls: Call[] = [];
  const queue = [...answers];

  t.mock.method(globalThis, "fetch", async (url: string | URL, init: RequestInit = {}): Promise<Response> => {
    calls.push({
      url: String(url),
      method: init.method ?? "GET",
      headers: (init.headers ?? {}) as Record<string, string>,
      body: init.body === undefined ? undefined : JSON.parse(init.body as string),
    });
    const next = queue.shift();
    if (next === undefined) throw new Error(`unexpected request #${calls.length}: ${init.method ?? "GET"} ${url}`);

    const body = typeof next.body === "string" ? next.body : next.body === undefined ? null : JSON.stringify(next.body);
    return new Response(body as string | null, { status: next.status, headers: next.headers });
  });

  return { calls };
}

/**
 * A successful payload must satisfy the tool's own output schema - the same check a client makes
 * on `structuredContent` - and lead with a summary sentence.
 */
export function expectOutput(TOOL: ToolDefinition, payload: Record<string, unknown>): void {
  const { summary, ...fields } = payload;
  assert.ok(typeof summary === "string", `${TOOL.name} has no summary`);
  assert.ok(summary.length > 0 && summary === summary.trim(), `${TOOL.name} summary is blank or padded`);

  const parsed = z.object(TOOL.outputSchema).safeParse(fields);
  assert.ok(parsed.success, `${TOOL.name} payload breaks its outputSchema: ${parsed.success ? "" : parsed.error.message}`);
}
