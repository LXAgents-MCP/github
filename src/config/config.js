/**
 * Every knob this server has, in one place.
 *
 * There are no command-line flags and no configuration file. The only input from outside the
 * tool arguments is two secrets and three settings, all variables: `GITHUB_API_KEY`, the token
 * every GitHub call is made with; `MCP_AUTH_TOKEN`, the secret a client must present to reach the
 * HTTP transport; and `MCP_TRANSPORT`, `PORT` and `HOST` (`src/routes/http.js`). A server without the
 * GitHub key still starts and says what is missing when a tool is used.
 */

/**
 * The environment variable that holds the GitHub token, on both transports. Never an argument, so
 * it never appears in a prompt.
 */
export const TOKEN_ENV = "GITHUB_API_KEY";

/**
 * The environment variable that holds the server's own access token for HTTP, named as
 * `shared-instruction` names its own. A client presents it as `Authorization: Bearer <token>`. It
 * is not a GitHub credential and is never sent to GitHub; stdio never reads it.
 */
export const AUTH_ENV = "MCP_AUTH_TOKEN";

/** Characters. The shortest access token HTTP will start with, as in `shared-instruction`. */
export const MIN_AUTH_TOKEN_LENGTH = 32;

/**
 * The port and interface of the HTTP transport when `PORT` and `HOST` are not set, the ones
 * `shared-instruction` uses: 3000 on every interface, so a container's published port reaches it. The
 * process holds a GitHub key, so what guards it is the access token it refuses to start without, and
 * TLS is the operator's. `HOST=127.0.0.1` narrows it to this machine.
 */
export const DEFAULT_PORT = 3000;
export const DEFAULT_HOST = "0.0.0.0";

/** Bytes. The largest request body the HTTP transport reads: a 1 MB file write is about 1.4 MB as base64. */
export const MAX_BODY_BYTES = 4_000_000;

/** The REST API. github.com only; GitHub Enterprise is not supported. */
export const API_BASE = "https://api.github.com";

/** Pinned on every request, so a change on GitHub's side cannot alter a response under us. */
export const API_VERSION = "2022-11-28";

/** Milliseconds. How long a call to GitHub may take before the tool gives up on it. */
export const API_TIMEOUT_MS = 30_000;

/** Bytes. The most of one answer the client will read; beyond it the call is refused. */
export const MAX_RESPONSE_BYTES = 20_000_000;

/** Characters. Past this the model reasons about a partial result as though it were complete. */
export const MAX_OUTPUT = 30_000;

/** Results per page when a call does not say, and the most GitHub allows. */
export const DEFAULT_PER_PAGE = 30;
export const MAX_PER_PAGE = 100;

/**
 * Characters of a file shown to the model. A file is read to be looked at, not ingested, and the
 * limit sits below `MAX_OUTPUT` so the summary and the fields still fit in the same result.
 */
export const MAX_FILE_CHARS = 25_000;

/** Bytes of content one write may carry. Source files fit; blobs are not what this is for. */
export const MAX_WRITE_BYTES = 1_000_000;

/** Entries shown for a directory. GitHub itself stops at 1000. */
export const MAX_LIST_ENTRIES = 500;

/** Characters of a diff shown to the model, which leaves room in a result for the rest of it. */
export const MAX_DIFF_CHARS = 20_000;

/** Changed files and commits listed in a result. The totals are always reported. */
export const MAX_FILES_SHOWN = 100;
export const MAX_COMMITS_SHOWN = 50;

/**
 * Characters of job logs shown by one `workflow_logs` call, shared between the jobs it reads, and
 * the most jobs it reads at once. A failing job's log ends with the reason, so the end is kept.
 */
export const MAX_LOG_CHARS = 20_000;
export const MAX_LOG_JOBS = 5;

/** Inputs one workflow dispatch may carry; GitHub refuses more. */
export const MAX_DISPATCH_INPUTS = 25;
