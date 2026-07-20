/**
 * Split text into segments marking which parts match any of the query terms,
 * for snippet highlighting. Case-insensitive, whole-substring per term. Pure —
 * the webview turns `mark: true` segments into <mark> elements (via textContent,
 * never innerHTML, so this never introduces markup injection).
 */
export interface Segment {
  text: string;
  mark: boolean;
}

export function highlightSegments(text: string, query: string): Segment[] {
  const terms = query
    .toLowerCase()
    .split(/[\s,]+/)
    .map((t) => t.replace(/^[+-]/, "").replace(/[*"]/g, ""))
    .filter((t) => t.length >= 2);
  if (terms.length === 0) {
    return [{ text, mark: false }];
  }

  const lower = text.toLowerCase();
  // Mark boolean per character, then coalesce into runs.
  const marked = new Array<boolean>(text.length).fill(false);
  for (const term of terms) {
    let from = 0;
    for (;;) {
      const idx = lower.indexOf(term, from);
      if (idx === -1) {
        break;
      }
      for (let i = idx; i < idx + term.length; i++) {
        marked[i] = true;
      }
      from = idx + term.length;
    }
  }

  const segments: Segment[] = [];
  let i = 0;
  while (i < text.length) {
    const mark = marked[i];
    let j = i;
    while (j < text.length && marked[j] === mark) {
      j++;
    }
    segments.push({ text: text.slice(i, j), mark });
    i = j;
  }
  return segments;
}

/**
 * Produce a short, single-line snippet centered on the first query match, so
 * the relevant part is visible without scrolling a long chunk.
 */
export function condenseSnippet(
  snippet: string,
  query: string,
  max = 160,
): string {
  const oneLine = snippet.replace(/\s*\n\s*/g, " ").trim();
  if (oneLine.length <= max) {
    return oneLine;
  }
  const terms = query
    .toLowerCase()
    .split(/[\s,]+/)
    .map((t) => t.replace(/^[+-]/, "").replace(/[*"]/g, ""))
    .filter((t) => t.length >= 2);
  const lower = oneLine.toLowerCase();
  let hit = -1;
  for (const term of terms) {
    const idx = lower.indexOf(term);
    if (idx !== -1 && (hit === -1 || idx < hit)) {
      hit = idx;
    }
  }
  if (hit === -1) {
    return `${oneLine.slice(0, max - 1)}…`;
  }
  const start = Math.max(0, hit - Math.floor(max / 3));
  const end = Math.min(oneLine.length, start + max);
  const prefix = start > 0 ? "…" : "";
  const suffix = end < oneLine.length ? "…" : "";
  return `${prefix}${oneLine.slice(start, end)}${suffix}`;
}
