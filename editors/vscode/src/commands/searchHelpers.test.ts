import { describe, expect, it } from "vitest";
import type { Capabilities } from "../sessfind/types";
import {
  availableMethods,
  isInstant,
  methodLabel,
  nextMethod,
  preferredMethod,
  projectName,
} from "./searchHelpers";

function caps(over: Partial<Capabilities["search_methods"]>): Capabilities {
  return {
    version: "0.9.0",
    json_api_version: 1,
    features: [],
    search_methods: {
      fts: true,
      fuzzy: true,
      semantic: false,
      llm: [],
      ...over,
    },
    data_dir: "/data",
  };
}

describe("availableMethods", () => {
  it("includes only supported methods, in order", () => {
    expect(availableMethods(caps({}))).toEqual(["fts", "fuzzy"]);
    expect(availableMethods(caps({ semantic: true, llm: ["claude"] }))).toEqual([
      "fts",
      "fuzzy",
      "semantic",
      "llm",
    ]);
  });

  it("always yields at least fts", () => {
    expect(
      availableMethods(caps({ fts: false, fuzzy: false })),
    ).toEqual(["fts"]);
  });
});

describe("preferredMethod", () => {
  it("honours a configured method that is available", () => {
    expect(preferredMethod("fuzzy", ["fts", "fuzzy"])).toBe("fuzzy");
  });

  it("falls back to the first when configured is unavailable", () => {
    expect(preferredMethod("semantic", ["fts", "fuzzy"])).toBe("fts");
    expect(preferredMethod(undefined, ["fuzzy", "fts"])).toBe("fuzzy");
  });
});

describe("nextMethod", () => {
  it("cycles with wraparound", () => {
    const m = ["fts", "fuzzy", "semantic"] as const;
    expect(nextMethod("fts", [...m])).toBe("fuzzy");
    expect(nextMethod("semantic", [...m])).toBe("fts");
  });
});

describe("isInstant", () => {
  it("classifies fts/fuzzy as instant and semantic/llm as deferred", () => {
    expect(isInstant("fts")).toBe(true);
    expect(isInstant("fuzzy")).toBe(true);
    expect(isInstant("semantic")).toBe(false);
    expect(isInstant("llm")).toBe(false);
  });
});

describe("methodLabel / projectName", () => {
  it("labels methods", () => {
    expect(methodLabel("fts")).toBe("Full-Text");
    expect(methodLabel("llm")).toBe("LLM");
  });

  it("takes the last path segment as project name", () => {
    expect(projectName("/home/me/my-repo")).toBe("my-repo");
    expect(projectName("C:\\Users\\me\\proj")).toBe("proj");
  });
});
