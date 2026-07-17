import esbuild from "esbuild";

const watch = process.argv.includes("--watch");

const options = {
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

if (watch) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
  console.log("esbuild: watching…");
} else {
  await esbuild.build(options);
  console.log("esbuild: build complete");
}
