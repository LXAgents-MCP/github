/** A request the owner would not want served, or one that cannot succeed. Carries a message worth reading. */
export class Refused extends Error {
  constructor(message: string) {
    super(message);
    this.name = "Refused";
  }
}

/**
 * What a caught value is read as. Anything can be thrown, so every field is optional and unknown,
 * and a reader falls back when one is missing.
 */
export type ErrorLike = {
  name?: unknown;
  message?: unknown;
  stack?: unknown;
  status?: unknown;
  cause?: { message?: unknown } | null;
} | null | undefined;
