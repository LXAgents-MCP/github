/** Read one tag, following an annotated tag to what it points at. */
import { z } from "zod";
import { github, parseRepo, refName, refPath } from "../services/api.js";
import { Refused } from "../utils/errors.js";
import { READ, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";
import { short } from "../utils/text.js";

export const TOOL = {
  name: "tag_get",
  description:
    "Read one tag of a GitHub repository: the commit it points at and, for an annotated tag, its " +
    "message, tagger and date. Use it to find the commit behind a version tag. tag_check only " +
    "says whether a tag exists.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag name, e.g. 'v1.2.0'."),
  },
  outputSchema: {
    tag: z.string(),
    sha: z.string(),
    commit_sha: z.string(),
    target_type: z.string(),
    annotated: z.boolean(),
    message: z.string().optional(),
    tagger: z.string().optional(),
    date: z.string().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");

  const ref = await github("GET", `${repo.path}/git/ref/tags/${refPath(tag)}`, {
    what: `read the tag '${tag}' of ${repo.full}`,
    explain: (status) =>
      status === 404 ? `There is no tag '${tag}' in ${repo.full}. List them with tag_get_all.` : undefined,
  });
  const object = ref.body?.object;
  if (!object || typeof object.sha !== "string") {
    throw new Refused("GitHub answered, but the body was not a tag reference this server recognizes.");
  }

  // A lightweight tag is only a reference to a commit. An annotated one is an object of its own,
  // which holds the message and the tagger and points at the commit.
  if (object.type !== "tag") {
    return {
      payload: {
        summary: `Tag ${tag} is a lightweight tag pointing at ${short(object.sha)}.`,
        tag,
        sha: object.sha,
        commit_sha: object.sha,
        target_type: object.type ?? "commit",
        annotated: false,
      },
      ok: true,
    };
  }

  const { body } = await github("GET", `${repo.path}/git/tags/${object.sha}`, {
    what: `read the annotated tag '${tag}' of ${repo.full}`,
  });
  expectBody(body?.object, "sha", "an annotated tag");
  return {
    payload: {
      summary: `Tag ${tag} is annotated and points at ${short(body.object.sha)}.`,
      tag,
      sha: object.sha,
      commit_sha: body.object.sha,
      target_type: body.object.type ?? "commit",
      annotated: true,
      ...(body.message ? { message: body.message } : {}),
      ...(body.tagger?.name ? { tagger: body.tagger.name } : {}),
      ...(body.tagger?.date ? { date: body.tagger.date } : {}),
    },
    ok: true,
  };
}
