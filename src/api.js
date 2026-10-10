/**
 * The GitHub client: addressing, requests, and the one place a failure becomes a message.
 *
 * Every tool reaches GitHub through `github()`. Nothing else builds a URL, so nothing else can
 * let an argument steer a call at a different endpoint: a repository, a reference and a file
 * path are validated and percent-encoded here and nowhere else.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { optionalInt, rawText, requiredText } from "./args.js";
import {
  API_BASE,
  API_TIMEOUT_MS,
  API_VERSION,
  DEFAULT_PER_PAGE,
  MAX_PER_PAGE,
  MAX_RESPONSE_BYTES,
  TOKEN_ENV,
} from "./config.js";
import { Refused } from "./errors.js";
import { VERSION } from "./version.js";

const JSON_ACCEPT = "application/vnd.github+json";

/* ------------------------------------------------------------------ the token */

/**
 * Read the API token from the environment, at call time.
 *
 * Read per call and not at startup, so a server without a token still starts and says what is
 * missing when a tool is used. The token comes from the `env` block of the client's
 * configuration for this server; it is never an argument, so it never appears in a prompt or a
 * transcript.
 *
 * @param {string} [envName]
 * @returns {string}
 */
export function requireToken(envName = TOKEN_ENV) {
  const token = process.env[envName]?.trim();
  if (!token) {
    throw new Refused(
      `${envName} is not set. Add it to the \`env\` block of this server's entry in the ` +
        "client configuration, then restart the client.",
    );
  }
  return token;
}

/**
 * Remove a secret from text that is about to be shown.
 *
 * @param {unknown} text
 * @param {string} secret
 * @returns {string}
 */
export function redact(text, secret) {
  return secret ? String(text).split(secret).join("***") : String(text);
}

/* ------------------------------------------------------------------ addressing */

// GitHub's own rules: a login is letters, digits and '-' (managed accounts add '_'), up to 39
// characters; a repository name is letters, digits, '-', '_' and '.', up to 100.
const OWNER = /^[A-Za-z0-9][A-Za-z0-9_-]{0,38}$/;
const REPO_NAME = /^[A-Za-z0-9._-]{1,100}$/;

/**
 * Read a repository argument, `owner/name`.
 *
 * @param {unknown} value
 * @param {string} [field]
 * @returns {{ owner: string, name: string, full: string, path: string }} `path` is the API prefix,
 *   `/repos/owner/name`, built only from characters that need no encoding.
 */
export function parseRepo(value, field = "repo") {
  const text = requiredText(value, field);
  const parts = text.split("/");
  const [owner, name] = parts;
  if (parts.length !== 2 || !OWNER.test(owner) || !REPO_NAME.test(name) || name === "." || name === "..") {
    throw new Refused(
      `\`${field}\` '${text}' is not a repository. Write it as 'owner/name', for example 'LXAgents-MCP/github'.`,
    );
  }
  return { owner, name, full: `${owner}/${name}`, path: `/repos/${owner}/${name}` };
}

// Characters git forbids in a reference name, plus whitespace and control characters.
const BAD_REF = /[\u0000- \u007f~^:?*[\\]/;

/**
 * Read a branch, tag or commit argument.
 *
 * Strict enough that the value cannot change what path it lands in - no `..`, no empty or
 * dotted segment, nothing git itself refuses - and loose enough to take any name git allows.
 *
 * @param {unknown} value
 * @param {string} field
 * @returns {string}
 */
export function refName(value, field) {
  const name = requiredText(value, field);
  const segments = name.split("/");
  if (
    name.length > 255 ||
    BAD_REF.test(name) ||
    name.includes("..") ||
    name.includes("@{") ||
    name.startsWith("-") ||
    name.endsWith(".") ||
    segments.some((segment) => segment === "" || segment.startsWith(".") || segment.endsWith(".lock"))
  ) {
    throw new Refused(`\`${field}\` '${name}' is not a valid branch, tag or commit name.`);
  }
  return name;
}

/**
 * A reference as it goes into a URL path: every segment encoded, the slashes kept, because
 * GitHub reads `feature/login` as one name spread over two segments.
 *
 * @param {string} name A name already accepted by `refName`.
 * @returns {string}
 */
export function refPath(name) {
  return name.split("/").map(encodeURIComponent).join("/");
}

/**
 * Read a path inside a repository.
 *
 * @param {unknown} value
 * @param {string} field
 * @param {{ allowEmpty?: boolean }} [options] `allowEmpty` makes the repository root a valid answer.
 * @returns {{ path: string, encoded: string }} The cleaned path, and the same as it goes into a URL.
 */
export function filePath(value, field, { allowEmpty = false } = {}) {
  const raw = rawText(value, field) ?? "";
  const trimmed = raw.trim().replace(/^\/+/, "").replace(/\/+$/, "");
  if (trimmed === "") {
    if (allowEmpty) return { path: "", encoded: "" };
    throw new Refused(`\`${field}\` is required: a path inside the repository, such as 'src/index.js'.`);
  }
  const segments = trimmed.split("/");
  if (/[\u0000-\u001f\u007f]/.test(trimmed) || segments.some((s) => s === "" || s === "." || s === "..")) {
    throw new Refused(
      `\`${field}\` '${trimmed}' is not a path inside the repository: no empty, '.' or '..' segments.`,
    );
  }
  return { path: trimmed, encoded: segments.map(encodeURIComponent).join("/") };
}

/* ------------------------------------------------------------------ paging */

/**
 * Read `page` and `per_page` from a tool's arguments.
 *
 * @param {{ page?: unknown, per_page?: unknown }} args
 * @returns {{ page: number, per_page: number }}
 */
export function pageQuery(args) {
  return {
    page: optionalInt(args.page, "page", { min: 1 }) ?? 1,
    per_page: optionalInt(args.per_page, "per_page", { min: 1, max: MAX_PER_PAGE }) ?? DEFAULT_PER_PAGE,
  };
}

/**
 * The paging fields of a list result: the page asked for, and whether GitHub has another.
 *
 * @param {{ page: number, per_page: number }} query
 * @param {Record<string, string>} headers The response headers, lowercased.
 * @returns {{ page: number, per_page: number, has_more: boolean }}
 */
export function paging(query, headers) {
  return { page: query.page, per_page: query.per_page, has_more: /rel="next"/.test(headers.link ?? "") };
}

/* ------------------------------------------------------------------ requests */

/**
 * Read a response body as text, refusing an answer larger than the client will read.
 *
 * @param {Response} response
 * @param {string} host
 * @returns {Promise<string>}
 */
async function readText(response, host) {
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0;

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Refused(`The answer from ${host} is larger than ${MAX_RESPONSE_BYTES / 1_000_000} MB. Ask for less.`);
    }
    chunks.push(value);
  }
  return Buffer.concat(chunks).toString("utf8");
}

/**
 * Call a hosting API and return its status, decoded body and headers.
 *
 * Every failure to get an answer - no network, a refused connection, a timeout - becomes a
 * `Refused` the model can read and act on. A non-2xx answer is not a failure here: the caller
 * knows what each status means. `token` is only used to scrub error text; the caller puts the
 * credential in `headers`.
 *
 * @param {string} url
 * @param {{ method?: string, headers?: Record<string, string>, json?: unknown, token?: string, raw?: boolean }} [options]
 *   `raw` keeps the whole body as text instead of decoding JSON.
 * @returns {Promise<{ status: number, body: any, headers: Record<string, string> }>}
 */
export async function callApi(url, { method = "GET", headers = {}, json, token = "", raw = false } = {}) {
  const host = new URL(url).host;
  const init = {
    method,
    headers: json === undefined ? headers : { ...headers, "Content-Type": "application/json" },
    signal: AbortSignal.timeout(API_TIMEOUT_MS),
  };
  if (json !== undefined) init.body = JSON.stringify(json);

  let response;
  let text;
  try {
    response = await fetch(url, init);
    text = await readText(response, host);
  } catch (error) {
    if (error instanceof Refused) throw error;
    if (error?.name === "TimeoutError" || error?.name === "AbortError") {
      throw new Refused(`${host} did not answer within ${API_TIMEOUT_MS / 1000} seconds. Try again.`);
    }
    const reason = redact(error?.cause?.message ?? error?.message ?? error, token);
    throw new Refused(`Could not reach ${host}: ${reason}. Check the network connection.`);
  }

  let body = null;
  if (text !== "") {
    if (raw) {
      body = text;
    } else {
      try {
        body = JSON.parse(text);
      } catch {
        body = text.slice(0, 300);
      }
    }
  }
  return { status: response.status, body, headers: Object.fromEntries(response.headers) };
}

/**
 * What GitHub said went wrong, as a short suffix: ": message (detail; detail)".
 *
 * @param {any} body
 * @returns {string}
 */
export function apiDetail(body) {
  if (typeof body === "string") return body ? `: ${body}` : "";
  if (!body || typeof body.message !== "string") return "";

  const extra = Array.isArray(body.errors)
    ? body.errors.map((e) => (typeof e === "string" ? e : (e?.message ?? e?.code))).filter(Boolean)
    : [];
  return `: ${body.message}` + (extra.length > 0 ? ` (${extra.join("; ")})` : "");
}

/** Is this answer GitHub saying "too many requests" rather than "you may not"? */
function isRateLimited(status, headers, body) {
  return (
    status === 429 ||
    headers["x-ratelimit-remaining"] === "0" ||
    headers["retry-after"] !== undefined ||
    /rate limit/i.test(apiDetail(body))
  );
}

/** When the limit lifts, in words the model can act on. */
function whenToRetry(headers) {
  const seconds = Number(headers["retry-after"]);
  if (Number.isFinite(seconds) && seconds > 0) return `Wait ${Math.ceil(seconds)} seconds and try again.`;

  const reset = Number(headers["x-ratelimit-reset"]);
  if (Number.isFinite(reset) && reset > 0) {
    const minutes = Math.max(1, Math.ceil((reset * 1000 - Date.now()) / 60_000));
    return `The limit resets at ${new Date(reset * 1000).toISOString()}, in about ${minutes} minute${minutes === 1 ? "" : "s"}.`;
  }
  return "Wait a minute and try again.";
}

/**
 * A refusal that says what to do next, for a status GitHub answered with.
 *
 * `explain` lets a tool word the statuses it knows better than the general case. It receives
 * the status and GitHub's own detail suffix, and returns a message, or `undefined` to fall
 * through to the general wording below.
 *
 * @returns {Refused}
 */
function failure({ status, body, headers, what, explain, token }) {
  const detail = apiDetail(body);
  let message = explain?.(status, detail);

  if (message === undefined) {
    if (status === 401) {
      message = `GitHub rejected the token (401). Check that ${TOKEN_ENV} holds a valid token that has not expired.`;
    } else if ((status === 403 || status === 429) && isRateLimited(status, headers, body)) {
      message = `GitHub is rate limiting this token (${status})${detail}. ${whenToRetry(headers)}`;
    } else if (status === 403) {
      const accepts = headers["x-accepted-github-permissions"];
      message =
        `GitHub refused to ${what} (403)${detail}. The token lacks permission for this: a classic ` +
        "token needs the matching scope (repo, workflow or delete_repo), a fine-grained token needs " +
        `the matching repository permission${accepts ? ` (GitHub says this endpoint accepts: ${accepts})` : ""}.`;
    } else if (status === 404) {
      message =
        `GitHub could not find what it needs to ${what} (404)${detail}. Check the spelling. GitHub ` +
        "also answers 404 for a private repository the token cannot see.";
    } else if (status === 409) {
      message = `GitHub could not ${what} (409)${detail}. Something changed or conflicts; read it again before retrying.`;
    } else if (status === 422) {
      message = `GitHub could not ${what} (422)${detail}.`;
    } else if (status >= 500) {
      message = `GitHub failed to ${what} (${status})${detail}. Try again shortly.`;
    } else {
      message = `GitHub answered ${status}${detail} when asked to ${what}.`;
    }
  }
  return new Refused(redact(message, token));
}

/**
 * Call GitHub and return the answer, or refuse with a message that names the next step.
 *
 * A 2xx answer is returned. So is any status listed in `allow`, for a call where that status is
 * an answer and not a failure - a tag that does not exist, for one. Everything else is refused.
 *
 * @param {string} method
 * @param {string} path Starts with `/` and is built only from `parseRepo`, `refPath`, `filePath`
 *   and numbers, so it needs no further encoding. No query string: use `query`.
 * @param {{
 *   query?: Record<string, unknown>,
 *   json?: unknown,
 *   accept?: string,
 *   allow?: number[],
 *   what?: string,
 *   explain?: (status: number, detail: string) => string | undefined,
 *   raw?: boolean,
 * }} [options] `what` completes "... to <what>" in a refusal.
 * @returns {Promise<{ status: number, body: any, headers: Record<string, string> }>}
 */
export async function github(method, path, options = {}) {
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
