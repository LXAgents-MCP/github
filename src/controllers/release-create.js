/** Create a release. */
import { z } from "zod";
import { github, parseRepo, refName } from "../services/api.js";
import { maybeFlag, optionalText, rawText } from "../models/args.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { describeRelease, expectBody, releaseFields, releaseShape } from "../models/shapes.js";

export const TOOL = {
  name: "release_create",
  description:
    "Create a release for a tag in a GitHub repository, creating the tag from a branch or commit if " +
    "it does not exist. Can be a draft or prerelease, and can have GitHub write the notes. This is " +
    "also how a tag is created.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag for the release, e.g. 'v1.2.0'. Created if it does not exist."),
    target: z
      .string()
      .optional()
      .describe("Branch or commit sha to create the tag from. Default: the default branch. Ignored if the tag exists."),
    name: z.string().optional().describe("Release title. Default: the tag."),
    body: z.string().optional().describe("Release notes, in Markdown."),
    draft: z.boolean().optional().describe("true creates an unpublished draft. Default false."),
    prerelease: z.boolean().optional().describe("true marks it as a prerelease. Default false."),
    generate_notes: z
      .boolean()
      .optional()
      .describe("true has GitHub write the notes from merged pull requests, added to any body given."),
  },
  outputSchema: releaseShape,
  annotations: WRITE,
};

/**
 * @param {{ repo?: unknown, tag?: unknown, target?: unknown, name?: unknown, body?: unknown,
 *   draft?: unknown, prerelease?: unknown, generate_notes?: unknown }} args
 */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");
  const target = args.target === undefined || args.target === null ? undefined : refName(args.target, "target");
  const name = optionalText(args.name, "name");
  const body = rawText(args.body, "body");
  const draft = maybeFlag(args.draft, "draft");
  const prerelease = maybeFlag(args.prerelease, "prerelease");
  const generate = maybeFlag(args.generate_notes, "generate_notes");

  const json = Object.fromEntries(
    Object.entries({
      tag_name: tag,
      target_commitish: target,
      name,
      body,
      draft,
      prerelease,
      generate_release_notes: generate,
    }).filter(([, value]) => value !== undefined),
  );
  const { body: created } = await github("POST", `${repo.path}/releases`, {
    json,
    what: `create a release for the tag '${tag}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 422 && /already_exists/i.test(detail)
        ? `GitHub could not create a release for '${tag}' in ${repo.full} (422)${detail}. A release for that tag ` +
          "already exists; read it with release_read, or use another tag."
        : undefined,
  });

  expectBody(created, "html_url", "a release");
  const fields = releaseFields(created);
  return { payload: { summary: `Created ${describeRelease(fields)} in ${repo.full}.`, ...fields }, ok: true };
}
