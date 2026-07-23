import esbuild from "esbuild";

const watch = process.argv.includes("--watch");

// Extension host bundle (Node).
const extensionOptions = {
  entryPoints: ["src/extension.ts"],
  bundle: true,
  outfile: "dist/extension.js",
  platform: "node",
  format: "cjs",
  target: "node18",
  // `vscode` is provided by the host at runtime, never bundled.
  external: ["vscode"],
  sourcemap: true,
  minify: !watch,
};

// Hub webview bundle (browser). The CSS import is extracted to dist/hub.css.
const webviewOptions = {
  entryPoints: [{ in: "src/hub/webview/main.ts", out: "hub" }],
  bundle: true,
  outdir: "dist",
  platform: "browser",
  format: "iife",
  target: "es2022",
  sourcemap: true,
  minify: !watch,
};

if (watch) {
  const ctxs = await Promise.all([
    esbuild.context(extensionOptions),
    esbuild.context(webviewOptions),
  ]);
  await Promise.all(ctxs.map((c) => c.watch()));
  console.log("esbuild: watching…");
} else {
  await Promise.all([
    esbuild.build(extensionOptions),
    esbuild.build(webviewOptions),
  ]);
  console.log("esbuild: build complete");
}
