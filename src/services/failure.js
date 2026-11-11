/**
 * The one place a status GitHub answered with becomes a message that names the next step.
 */
import { TOKEN_ENV } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { redact } from "../utils/redact.js";

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
export function failure({ status, body, headers, what, explain, token }) {
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
