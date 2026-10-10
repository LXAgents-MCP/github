/**
 * Every knob this server has, in one place.
 *
 * There are no command-line flags and no configuration file. The only input from outside the
 * tool arguments is the token and, to serve HTTP, three variables: `MCP_AUTH_TOKEN` over stdio,
 * the `Authorization` header over HTTP (`src/auth.js`), and `MCP_TRANSPORT`, `PORT` and `HOST`
 * (`src/http.js`). A server without a token still starts and says what is missing when asked.
 */

/**
 * The environment variable that holds the token over stdio, named as `shared-instruction` names its
 * own token. Never an argument, so it never appears in a prompt. HTTP never reads it.
 */
export const TOKEN_ENV = "MCP_AUTH_TOKEN";

/** The port and interface of the HTTP transport when `PORT` and `HOST` are not set. Loopback, because the token is a GitHub credential. */
export const DEFAULT_PORT = 3001;
export const DEFAULT_HOST = "127.0.0.1";

/** Characters. The longest bearer token accepted; GitHub's longest format is well under it. */
export const MAX_TOKEN_LENGTH = 512;

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
