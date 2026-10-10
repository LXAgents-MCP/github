/** Does a tag exist? */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../api.js";
import { READ, repoArg } from "../schemas.js";
import { short } from "../text.js";

export const TOOL = {
  name: "tag_check",
  description:
    "Check whether a tag exists in a GitHub repository, and if it does, what it points at and whether " +
    "it is annotated. A missing tag is an answer, not an error. Use it before creating a release, or " +
    "to avoid reusing a tag name.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag name, e.g. 'v1.2.0'."),
  },
  outputSchema: {
    exists: z.boolean(),
    tag: z.string(),
    sha: z.string().optional(),
    annotated: z.boolean().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");

  // A tag that is not there is the answer to this call, so a 404 is returned and not refused.
  const { status, body } = await github("GET", `${repo.path}/git/ref/tags/${refPath(tag)}`, {
    allow: [404],
    what: `look for the tag '${tag}' in ${repo.full}`,
  });

  if (status === 404 || !body?.object?.sha) {
    return {
      payload: { summary: `Tag ${tag} does not exist in ${repo.full}.`, exists: false, tag },
      ok: true,
    };
  }
  const annotated = body.object.type === "tag";
  return {
    payload: {
      summary: `Tag ${tag} exists in ${repo.full} (${annotated ? "annotated" : "lightweight"}) and points at ${short(body.object.sha)}.`,
      exists: true,
      tag,
      sha: body.object.sha,
      annotated,
    },
    ok: true,
  };
}
