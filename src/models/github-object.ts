/**
 * A JSON object as GitHub returns it.
 *
 * GitHub's answers are read defensively, field by field, with a fallback for every field that may
 * be missing, and a proxy or an outage can answer with something else entirely. Typing each object
 * in full would claim a shape this server never checks, so an object is a record of unknown-shaped
 * fields and each tool reads only what it reports.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type GitHubObject = Record<string, any>;
