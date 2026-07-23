/**
 * POSIX-quote a single argument so it survives a shell round-trip when written
 * to a terminal via `Terminal.sendText`. Wraps in single quotes and escapes any
 * embedded single quotes. Safe default for macOS/Linux shells; Windows
 * PowerShell quoting differs and is handled separately (roadmap).
 */
export function quoteArg(arg: string): string {
  if (arg.length > 0 && /^[A-Za-z0-9_./:=-]+$/.test(arg)) {
    return arg; // no metacharacters, leave bare for readability
  }
  return `'${arg.replace(/'/g, `'\\''`)}'`;
}

/** Join a command spec's args into a single shell-safe command line. */
export function quoteCommand(args: string[]): string {
  return args.map(quoteArg).join(" ");
}
