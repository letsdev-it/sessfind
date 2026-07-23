import type { SessionDetail } from "../sessfind/types";

/**
 * Render a session detail as Markdown. Pure (no `vscode` import) so it can be
 * unit-tested and reused by the document provider.
 */
export function renderSession(detail: SessionDetail): string {
  const s = detail.session;
  const lines: string[] = [];
  const title = s.title ?? s.session_id;
  lines.push(`# ${title}`, "");
  lines.push(`- **Source:** ${s.source}`);
  lines.push(`- **Project:** \`${s.project}\``);
  lines.push(`- **Date:** ${formatDate(s.timestamp)}`);
  lines.push(`- **Session ID:** \`${s.session_id}\``);
  if (s.tags.length > 0) {
    lines.push(`- **Tags:** ${s.tags.map((t) => `\`${t}\``).join(", ")}`);
  }
  lines.push("", "---", "");

  for (const chunk of detail.chunks) {
    for (const line of chunk.snippet.split("\n")) {
      if (line.startsWith("USER:")) {
        lines.push("", "### User", "", line.slice("USER:".length).trim());
      } else if (line.startsWith("ASSISTANT:")) {
        lines.push(
          "",
          "### Assistant",
          "",
          line.slice("ASSISTANT:".length).trim(),
        );
      } else if (line.startsWith("[tools:")) {
        lines.push("", `> ${line}`);
      } else {
        lines.push(line);
      }
    }
    lines.push("");
  }

  return lines.join("\n");
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}
