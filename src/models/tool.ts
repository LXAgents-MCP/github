/**
 * The contract between a tool module in `src/controllers/` and the server that serves it.
 *
 * A module exports `TOOL`, a `ToolDefinition`, and `handle(args)`, resolving to a `ToolResult`. The
 * registry (`src/controllers/index.ts`) pairs the two into a `ToolSpec`; the protocol layer
 * (`src/routes/server.ts`) only ever sees `ToolSpec`s.
 */
import type { ToolAnnotations } from "@modelcontextprotocol/sdk/types.js";
import type { ZodType } from "zod";

/** A tool's arguments or result, one schema per field. */
export type Shape = Record<string, ZodType>;

/** What a client reads about a tool in `tools/list`. */
export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: Shape;
  outputSchema: Shape;
  annotations: ToolAnnotations;
}

/** A tool's answer: a one-sentence `summary`, and fields that satisfy its `outputSchema`. */
export type Payload = { summary: string } & Record<string, unknown>;

/** What `handle` resolves to. A failure is a thrown `Refused`, not a result. */
export interface ToolResult {
  payload: Payload;
  ok: boolean;
}

/** A tool's arguments as the client sent them, before the tool has checked any of them. */
export type ToolArgs = Record<string, unknown>;

/** One tool as the registry holds it. */
export interface ToolSpec {
  name: string;
  TOOL: ToolDefinition;
  handle: (args: ToolArgs) => Promise<ToolResult>;
}
