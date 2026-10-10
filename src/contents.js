/**
 * The repository contents API, which the file tools share.
 *
 * GitHub changes a file by `PUT` or `DELETE` against its path and wants the blob sha of the
 * version being replaced, so a tool that is not handed one has to read it first. That read, the
 * path, and the encoding of what is written live here once.
 */
import { github } from "./api.js";
import { MAX_WRITE_BYTES } from "./config.js";
import { Refused } from "./errors.js";

/**
 * The API path of a file or directory.
 *
 * @param {{ path: string }} repo
 * @param {{ encoded: string }} file An already-encoded path, from `filePath`; empty for the root.
 * @returns {string}
 */
export function contentsPath(repo, file) {
  return file.encoded === "" ? `${repo.path}/contents` : `${repo.path}/contents/${file.encoded}`;
}

/**
 * Content as the API wants it written, refusing what is too large to be a source file.
 *
 * @param {string} content
 * @returns {string} Base64.
 */
export function encodeContent(content) {
  const bytes = Buffer.from(content, "utf8");
  if (bytes.length > MAX_WRITE_BYTES) {
    throw new Refused(
      `\`content\` is ${bytes.length} bytes, over the ${MAX_WRITE_BYTES}-byte limit for one write. ` +
        "This tool writes source files; commit large files with git.",
    );
  }
  return bytes.toString("base64");
}

/**
 * The blob sha of the file at `path`, read so a change can say which version it replaces.
 *
 * @param {{ full: string, path: string }} repo
 * @param {{ path: string, encoded: string }} file
 * @param {string | undefined} ref The branch to read it on; undefined is the default branch.
 * @param {string} instead What to suggest when there is no such file.
 * @returns {Promise<string>}
 */
export async function blobSha(repo, file, ref, instead) {
  const { body } = await github("GET", contentsPath(repo, file), {
    query: { ref },
    what: `read '${file.path}' in ${repo.full} to find the version it replaces`,
    explain: (status) =>
      status === 404 ? `There is no file '${file.path}' in ${repo.full}${ref ? ` on ${ref}` : ""}. ${instead}` : undefined,
  });

  if (Array.isArray(body)) {
    throw new Refused(`\`path\` '${file.path}' is a directory in ${repo.full}, not a file.`);
  }
  if (!body || body.type !== "file" || typeof body.sha !== "string") {
    throw new Refused(`\`path\` '${file.path}' in ${repo.full} is not a regular file, so it cannot be replaced.`);
  }
  return body.sha;
}
