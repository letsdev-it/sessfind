import type { Capabilities, SearchMethod } from "../sessfind/types";

export const INSTANT_METHODS: SearchMethod[] = ["fts", "fuzzy"];

/** Methods the binary can actually serve, in display order. */
export function availableMethods(caps: Capabilities): SearchMethod[] {
  const m: SearchMethod[] = [];
  if (caps.search_methods.fts) {
    m.push("fts");
  }
  if (caps.search_methods.fuzzy) {
    m.push("fuzzy");
  }
  if (caps.search_methods.semantic) {
    m.push("semantic");
  }
  if (caps.search_methods.llm.length > 0) {
    m.push("llm");
  }
  return m.length > 0 ? m : ["fts"];
}

/** The configured default if available, else the first available method. */
export function preferredMethod(
  configured: string | undefined,
  methods: SearchMethod[],
): SearchMethod {
  const wanted = configured as SearchMethod | undefined;
  return wanted && methods.includes(wanted) ? wanted : methods[0];
}

/** Next method in the cycle, wrapping around. */
export function nextMethod(
  current: SearchMethod,
  methods: SearchMethod[],
): SearchMethod {
  return methods[(methods.indexOf(current) + 1) % methods.length];
}

export function isInstant(method: SearchMethod): boolean {
  return INSTANT_METHODS.includes(method);
}

export function methodLabel(method: SearchMethod): string {
  switch (method) {
    case "fts":
      return "Full-Text";
    case "fuzzy":
      return "Fuzzy";
    case "semantic":
      return "Semantic";
    case "llm":
      return "LLM";
  }
}

export function firstLine(text: string): string {
  return text.split("\n")[0]?.trim() ?? "";
}

export function projectName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
