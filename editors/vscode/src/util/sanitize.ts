/**
 * Strip characters that would break a URI path or read oddly in an editor tab
 * title, collapse whitespace, and cap the length.
 */
export function sanitizeForPath(name: string): string {
  return name
    .replace(/[\\/:*?"<>|#%]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 60);
}
