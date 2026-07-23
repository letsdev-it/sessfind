/** Split a free-text tag input on commas/whitespace into distinct tags. */
export function splitTags(input: string | undefined): string[] {
  if (!input) {
    return [];
  }
  return input
    .split(/[,\s]+/)
    .map((t) => t.trim())
    .filter((t) => t.length > 0);
}
