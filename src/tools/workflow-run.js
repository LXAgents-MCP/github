/** Start a GitHub Actions workflow by dispatch. */
import { z } from "zod";
import { github, parseRepo, refName, workflowName } from "../api.js";
import { MAX_DISPATCH_INPUTS } from "../config.js";
import { Refused } from "../errors.js";
import { WRITE, repoArg } from "../schemas.js";

/**
 * The inputs of a dispatch. GitHub takes every value as a string, as the workflow file reads them,
 * so a number or a flag is written out and checked by the workflow, not here.
 *
 * @param {unknown} value
 * @returns {Record<string, string> | undefined}
 */
function dispatchInputs(value) {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new Refused("`inputs` must be an object of input names and their values.");
  }
  const entries = Object.entries(value);
  if (entries.length > MAX_DISPATCH_INPUTS) {
    throw new Refused(`\`inputs\` may hold at most ${MAX_DISPATCH_INPUTS} inputs, got ${entries.length}.`);
  }
  const inputs = {};
  for (const [key, entry] of entries) {
    if (key.trim() === "") throw new Refused("`inputs` has an input with a blank name.");
    if (!["string", "number", "boolean"].includes(typeof entry)) {
      throw new Refused(`\`inputs.${key}\` must be a string, a number or true/false.`);
    }
    inputs[key] = String(entry);
  }
  return entries.length === 0 ? undefined : inputs;
}

export const TOOL = {
  name: "workflow_run",
  description:
    "Start a GitHub Actions workflow run on a branch or tag (default: the default branch), with optional " +
    "inputs. It runs whatever the workflow does, which may deploy or publish. The workflow needs an " +
    "'on: workflow_dispatch' trigger and its file must be on the default branch. Returns the run id when " +
    "GitHub gives it; follow the run with workflow_status.",
  inputSchema: {
    repo: repoArg,
    workflow: z.string().describe("Workflow id or file name, e.g. 'ci.yml'. See workflow_list."),
    ref: z.string().optional().describe("Branch or tag to run it on. Default: the repository's default branch."),
    inputs: z
      .record(z.string(), z.union([z.string(), z.number(), z.boolean()]))
      .optional()
      .describe("The workflow's inputs by name, up to 25. Values are sent as text."),
  },
  outputSchema: {
    dispatched: z.boolean(),
    workflow: z.string(),
    ref: z.string(),
    run_id: z.number().optional(),
    html_url: z.string().optional(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, workflow?: unknown, ref?: unknown, inputs?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const workflow = workflowName(args.workflow);
  const given = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");
  const inputs = dispatchInputs(args.inputs);

  const ref =
    given ?? (await github("GET", repo.path, { what: `find the default branch of ${repo.full}` })).body?.default_branch;
  if (typeof ref !== "string") throw new Refused(`Could not find the default branch of ${repo.full}; pass \`ref\`.`);

  // Asking for the run's details is what makes GitHub say which run it started; without it the
  // answer is empty. An answer with no body is handled below, so the call works either way.
  const { body } = await github("POST", `${repo.path}/actions/workflows/${workflow.encoded}/dispatches`, {
    json: { ref, ...(inputs === undefined ? {} : { inputs }), return_run_details: true },
    what: `run the workflow '${workflow.name}' of ${repo.full} on '${ref}'`,
    explain: (status, detail) => {
      if (status === 404) {
        return (
          `GitHub could not find the workflow '${workflow.name}' in ${repo.full} (404)${detail}. Check it with ` +
          "workflow_list: the workflow file must exist on the repository's default branch. GitHub also answers " +
          "404 for a private repository the token cannot see."
        );
      }
      if (status === 422) {
        return (
          `GitHub would not start '${workflow.name}' on '${ref}' in ${repo.full} (422)${detail}. The workflow needs ` +
          "an 'on: workflow_dispatch' trigger, every input it requires must be given, and no input it does not declare."
        );
      }
      return undefined;
    },
  });

  const runId = Number.isInteger(body?.workflow_run_id) ? body.workflow_run_id : undefined;
  const url = typeof body?.html_url === "string" ? body.html_url : undefined;
  return {
    payload: {
      summary:
        runId === undefined
          ? `Asked GitHub to run workflow '${workflow.name}' on ${ref} in ${repo.full}. GitHub did not say which run ` +
            "it started; find it with workflow_status in a few seconds."
          : `Started workflow '${workflow.name}' on ${ref} in ${repo.full}: run ${runId}.`,
      dispatched: true,
      workflow: workflow.name,
      ref,
      ...(runId === undefined ? {} : { run_id: runId }),
      ...(url === undefined ? {} : { html_url: url }),
    },
    ok: true,
  };
}
