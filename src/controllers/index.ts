/**
 * Tool registry: one module per tool, discovered rather than listed.
 *
 * Adding a tool is a new file in this directory that exports `TOOL` (name, description and
 * schemas) and `handle(args)`, resolving to `{ payload, ok }`. The file name is the tool name in
 * kebab case - `repo-create.ts` is `repo_create` - and a test holds the two together in both
 * directions. Nothing needs editing elsewhere: `src/routes/server.ts` never names a tool, so the
 * protocol layer cannot grow a per-tool special case. A module that does not export both is
 * skipped, so a helper dropped here is never callable as a tool.
 */
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import type { ToolDefinition, ToolSpec } from "../models/tool.js";

export type { ToolSpec } from "../models/tool.js";

const HERE = dirname(fileURLToPath(import.meta.url));

/**
 * Whether `file` is a module that may hold a tool: TypeScript when the server runs from `src/`,
 * JavaScript when it runs from the compiled `dist/`. Declaration files and this registry are not.
 *
 * @param file A file name, without its directory.
 */
export function isToolModule(file: string): boolean {
  return /\.[jt]s$/.test(file) && !file.endsWith(".d.ts") && !/^index\.[jt]s$/.test(file);
}

/**
 * Import every tool module in `dir`.
 *
 * @param dir Defaults to this directory; a different one is for tests.
 */
export async function discoverTools(dir: string = HERE): Promise<Map<string, ToolSpec>> {
  const found = new Map<string, ToolSpec>();
  const files = readdirSync(dir).filter(isToolModule).sort();

  for (const file of files) {
    const { TOOL, handle } = (await import(pathToFileURL(join(dir, file)).href)) as Partial<ToolModule>;
    if (TOOL === undefined || typeof handle !== "function") continue;

    if (!TOOL.name) throw new Error(`${file} TOOL has no 'name'.`);
    if (found.has(TOOL.name)) {
      // Two tools answering to one name makes tools/call dispatch arbitrary; fail loudly at
      // startup rather than silently at call time.
      throw new Error(`Duplicate tool name '${TOOL.name}' in ${file}.`);
    }
    found.set(TOOL.name, { name: TOOL.name, TOOL, handle });
  }

  return found;
}

/** What a tool module exports; either may be missing from a module that is not a tool. */
interface ToolModule {
  TOOL: ToolDefinition;
  handle: ToolSpec["handle"];
}

export const REGISTRY = await discoverTools();

export const TOOLS: ToolSpec[] = [...REGISTRY.values()];

export function get(name: string): ToolSpec | undefined {
  return REGISTRY.get(name);
}
