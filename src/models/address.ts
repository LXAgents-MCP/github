/**
 * Reading the arguments that end up in a GitHub URL: a repository, a reference, a path in a
 * repository, a workflow. Each is validated here and returned with the form it takes in a URL, so
 * `src/services/api.ts` can build a path from nothing that has not passed through this file.
 */
import { Refused } from "../utils/errors.js";
import { rawText, requiredText } from "./args.js";

// GitHub's own rules: a login is letters, digits and '-' (managed accounts add '_'), up to 39
// characters; a repository name is letters, digits, '-', '_' and '.', up to 100.
const OWNER = /^[A-Za-z0-9][A-Za-z0-9_-]{0,38}$/;

export const REPO_NAME = /^[A-Za-z0-9._-]{1,100}$/;

/**
 * Read a repository argument, `owner/name`.
 *
 * @returns `path` is the API prefix,
 *   `/repos/owner/name`, built only from characters that need no encoding.
 */
export function parseRepo(value: unknown, field: string = "repo"): { owner: string; name: string; full: string; path: string } {
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
 */
export function refName(value: unknown, field: string): string {
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
 * @param name A name already accepted by `refName`.
 */
export function refPath(name: string): string {
  return name.split("/").map(encodeURIComponent).join("/");
}

/**
 * Read a path inside a repository.
 *
 * @param options `allowEmpty` makes the repository root a valid answer.
 * @returns The cleaned path, and the same as it goes into a URL.
 */
export function filePath(value: unknown, field: string, { allowEmpty = false }: { allowEmpty?: boolean } = {}): { path: string; encoded: string } {
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

/**
 * Read a workflow argument: its numeric id, or the name of its file under `.github/workflows/`.
 *
 * A path is accepted and reduced to the file name, because that is how a workflow is written
 * everywhere else. Nothing else can reach a URL: no slash, and a name starts with a letter or a digit.
 *
 * @returns The cleaned value, and the same as it goes into a URL.
 */
export function workflowName(value: unknown, field: string = "workflow"): { name: string; encoded: string } {
  const name = requiredText(value, field).replace(/^\.github\/workflows\//, "");
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$/.test(name)) {
    throw new Refused(
      `\`${field}\` '${name}' is not a workflow. Give its numeric id, or the name of its file such as 'ci.yml' (see workflow_list).`,
    );
  }
  return { name, encoded: encodeURIComponent(name) };
}
