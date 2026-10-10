/** Ask people or teams to review a pull request. */
import { z } from "zod";
import { github, parseRepo } from "../api.js";
import { requiredInt, textList } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody, namesOf } from "../shapes.js";

export const TOOL = {
  name: "pr_review_request",
  description:
    "Request a review of a pull request in a GitHub repository from people, teams, or both. Reviewers " +
    "must be collaborators and cannot be the pull request's author. Give at least one.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Pull request number."),
    reviewers: z.array(z.string()).optional().describe("GitHub logins to ask."),
    team_reviewers: z.array(z.string()).optional().describe("Team slugs to ask, e.g. 'platform'."),
  },
  outputSchema: {
    number: z.number(),
    reviewers: z.array(z.string()),
    teams: z.array(z.string()),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, reviewers?: unknown, team_reviewers?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const reviewers = textList(args.reviewers, "reviewers");
  const teams = textList(args.team_reviewers, "team_reviewers");
  if ((reviewers?.length ?? 0) + (teams?.length ?? 0) === 0) {
    throw new Refused("Give at least one of `reviewers` and `team_reviewers`.");
  }

  const { body } = await github("POST", `${repo.path}/pulls/${number}/requested_reviewers`, {
    json: {
      ...(reviewers?.length ? { reviewers } : {}),
      ...(teams?.length ? { team_reviewers: teams } : {}),
    },
    what: `request a review of pull request #${number} of ${repo.full}`,
    explain: (status, detail) =>
      status === 422
        ? `GitHub would not request that review on #${number} of ${repo.full} (422)${detail}. Reviewers must be ` +
          "collaborators with read access, and the pull request's author cannot review their own work."
        : undefined,
  });

  expectBody(body, "html_url", "a pull request");
  const people = namesOf(body.requested_reviewers);
  const slugs = (Array.isArray(body.requested_teams) ? body.requested_teams : []).map((team) => team.slug ?? team.name);
  const asked = [...people, ...slugs.map((slug) => `team ${slug}`)];
  return {
    payload: {
      summary: `Review requested on #${number} in ${repo.full} from: ${asked.length > 0 ? asked.join(", ") : "nobody"}.`,
      number,
      reviewers: people,
      teams: slugs,
    },
    ok: true,
  };
}
