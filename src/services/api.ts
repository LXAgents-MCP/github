/**
 * The GitHub client: the one place a GitHub URL is built and a call is made.
 *
 * Every tool reaches GitHub through `github()`, and nothing else builds a URL, so nothing else can
 * let an argument steer a call at a different endpoint. The parts of a path come only from
 * `src/models/address.ts`, which validates and percent-encodes them; the request itself is
 * `src/services/client.ts`, and a refusal's wording is `src/services/failure.ts`.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { API_BASE, API_VERSION } from "../config/config.js";
import { VERSION } from "../config/version.js";
import { requireToken } from "./auth.js";
import { callApi, type ApiAnswer } from "./client.js";
import { failure, type Explain } from "./failure.js";

const JSON_ACCEPT = "application/vnd.github+json";

/** How a tool asks for one GitHub call. */
export interface GitHubOptions {
  query?: Record<string, unknown>;
  json?: unknown;
  accept?: string;
  allow?: number[];
  what?: string;
  explain?: Explain;
  raw?: boolean;
}

/**
 * Call GitHub and return the answer, or refuse with a message that names the next step.
 *
 * A 2xx answer is returned. So is any status listed in `allow`, for a call where that status is
 * an answer and not a failure - a tag that does not exist, for one. Everything else is refused.
 *
 * @param path Starts with `/` and is built only from `parseRepo`, `refPath`, `filePath`
 *   and numbers, so it needs no further encoding. No query string: use `query`.
 * @param options `what` completes "... to <what>" in a refusal.
 */
export async function github(method: string, path: string, options: GitHubOptions = {}): Promise<ApiAnswer> {
  const { query, json, accept = JSON_ACCEPT, allow = [], what = "complete the request", explain, raw = false } = options;
  if (!path.startsWith("/") || /[?#]/.test(path)) {
    throw new Error(`github(): path must start with '/' and carry no query or fragment, got '${path}'.`);
  }

  const token = requireToken();
  const url = new URL(`${API_BASE}${path}`);
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") url.searchParams.set(key, String(value));
  }

  const answer = await callApi(url.href, {
    method,
    token,
    json,
    raw,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: accept,
      "X-GitHub-Api-Version": API_VERSION,
      "User-Agent": `lxagents-mcp-github/${VERSION}`,
    },
  });

  if ((answer.status >= 200 && answer.status < 300) || allow.includes(answer.status)) return answer;
  throw failure({ ...answer, what, explain, token });
}
