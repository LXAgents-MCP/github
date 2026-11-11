/** The paging arguments every list tool takes, and the paging fields it returns. */
import { DEFAULT_PER_PAGE, MAX_PER_PAGE } from "../config/config.js";
import { optionalInt } from "./args.js";

/**
 * Read `page` and `per_page` from a tool's arguments.
 */
export function pageQuery(args: { page?: unknown; per_page?: unknown }): { page: number; per_page: number } {
  return {
    page: optionalInt(args.page, "page", { min: 1 }) ?? 1,
    per_page: optionalInt(args.per_page, "per_page", { min: 1, max: MAX_PER_PAGE }) ?? DEFAULT_PER_PAGE,
  };
}

/**
 * The paging fields of a list result: the page asked for, and whether GitHub has another.
 *
 * @param headers The response headers, lowercased.
 */
export function paging(query: { page: number; per_page: number }, headers: Record<string, string>): { page: number; per_page: number; has_more: boolean } {
  return { page: query.page, per_page: query.per_page, has_more: /rel="next"/.test(headers.link ?? "") };
}
