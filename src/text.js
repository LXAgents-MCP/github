/** Small text helpers shared by the tools. Pure functions, no I/O. */

/** The abbreviated form of a commit sha. */
export function short(sha) {
  return typeof sha === "string" ? sha.slice(0, 7) : "";
}

/** The first line of a message, which is all a listing needs. */
export function firstLine(message) {
  return typeof message === "string" ? message.split(/\r?\n/, 1)[0] : "";
}

/**
 * Decode bytes as text: UTF-8 when they are valid UTF-8, Windows-1252 when they are not.
 *
 * The byte order mark is kept (`ignoreBOM`), so a caller that cares can strip it explicitly. With
 * `max`, only the first `max` bytes are decoded, and a character the cut split is dropped
 * instead of shown as a replacement mark.
 *
 * @param {Uint8Array} bytes
 * @param {{ max?: number }} [options]
 * @returns {{ text: string, truncated: boolean }}
 */
export function decodeText(bytes, { max } = {}) {
  let encoding = "utf-8";
  try {
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    encoding = "windows-1252";
  }

  const truncated = max !== undefined && bytes.length > max;
  let text = new TextDecoder(encoding, { ignoreBOM: true }).decode(truncated ? bytes.subarray(0, max) : bytes);
  if (truncated && encoding === "utf-8") text = text.replace(/\uFFFD$/, "");
  return { text, truncated };
}

/** Does this look like binary data? A NUL byte early on is the usual tell. */
export function looksBinary(bytes) {
  return bytes.subarray(0, 8000).includes(0);
}

/**
 * Cut text to a length, keeping the start.
 *
 * @param {string} text
 * @param {number} max
 * @returns {[string, boolean]} The text and whether it was cut.
 */
export function head(text, max) {
  return text.length > max ? [`${text.slice(0, max)}\n... (truncated)`, true] : [text, false];
}

/**
 * Cut text to a length, keeping the end - where a log says why it failed.
 *
 * @param {string} text
 * @param {number} max
 * @returns {[string, boolean]} The text and whether it was cut.
 */
export function tail(text, max) {
  return text.length > max ? [`(truncated) ...\n${text.slice(text.length - max)}`, true] : [text, false];
}
