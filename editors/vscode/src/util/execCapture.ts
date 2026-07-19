import { execFile } from "node:child_process";

/**
 * Run a command and capture stdout; resolves to null on any failure (missing
 * binary, non-zero exit, timeout). Used for best-effort enrichment like git
 * history — the caller renders nothing when null.
 */
export function execCapture(
  command: string,
  args: string[],
  cwd: string,
  timeoutMs = 3000,
): Promise<string | null> {
  return new Promise((resolve) => {
    execFile(
      command,
      args,
      { cwd, timeout: timeoutMs, maxBuffer: 1024 * 1024 },
      (err, stdout) => {
        resolve(err ? null : stdout.toString().trim());
      },
    );
  });
}
