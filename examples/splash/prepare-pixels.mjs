// Build a development preview with the upstream browser renderer, without npm.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const revision = "a98915a9fa0d0d74c8a2094935a02de4425e9b1d";
const width = 800, height = 400, fps = 15, count = 30;

function main() {
  if (process.argv.length !== 4) {
    throw new Error("Usage: node examples/splash/prepare-pixels.mjs <ascii-checkout> <new-output-directory>");
  }
  const upstream = resolve(process.argv[2]);
  const output = resolve(process.argv[3]);
  const actual = execFileSync("git", ["-C", upstream, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  if (actual !== revision) throw new Error(`Expected ascii revision ${revision}; found ${actual}.`);
  execFileSync("git", ["-C", upstream, "diff", "--quiet", "HEAD", "--", "src/mount.ts", "src/pieces/black-prism.ts"]);

  const moduleUrl = (file) => {
    const javascript = stripTypeScriptTypes(readFileSync(resolve(upstream, file), "utf8"));
    return "data:text/javascript;base64," + Buffer.from(javascript).toString("base64");
  };
  const imports = `import * as piece from ${JSON.stringify(moduleUrl("src/pieces/black-prism.ts"))};
import { mount } from ${JSON.stringify(moduleUrl("src/mount.ts"))};`;
  mkdirSync(dirname(output), { recursive: true });
  mkdirSync(output); // Refuse to mix new frames with an existing preview.
  mkdirSync(resolve(output, "frames"));
  mkdirSync(resolve(output, "firefox-profile"));
  copyFileSync(resolve(upstream, "LICENSE"), resolve(output, "LICENSE.ascii"));

  writeFileSync(resolve(output, "reference.html"), `<!doctype html>
<meta charset="utf-8"><title>black-prism Canvas reference</title>
<style>body{margin:0;background:#000;color:#bbb;font:14px monospace;text-align:center}
canvas{display:block;width:min(800px,100%);margin:auto}button{margin:16px}</style>
<canvas id="art"></canvas><button id="replay">Replay 2 seconds</button>
<p>Original Canvas renderer, fixed upstream revision. Terminal preview is separate.</p>
<script type="module">
${imports}
let stop = () => {}, timer;
function play() {
  stop(); clearTimeout(timer);
  stop = mount(document.querySelector('#art'), piece, { motion: true });
  timer = setTimeout(() => stop(), 2000);
}
document.querySelector('#replay').onclick = play;
play();
</script>`);

  // Freeze the original renderer at exact source times, one canvas per frame.
  const sheet = resolve(output, "sheet.html");
  writeFileSync(sheet, `<!doctype html><meta charset="utf-8">
<style>html,body{margin:0;padding:0;background:#000;overflow:hidden}
body{display:grid;grid-template-columns:repeat(5,${width}px)}
canvas{display:block;width:${width}px;height:${height}px}</style>
<script type="module">
${imports}
const frame = piece.default();
for (let i = 0; i < ${count}; i++) {
  const canvas = document.createElement('canvas');
  document.body.append(canvas);
  mount(canvas, { meta: { ...piece.meta, fps: 0 }, default: () => (_, env) => frame(i / ${fps}, env) });
}
document.title = 'frames-ready';
</script>`);

  execFileSync("firefox", ["--headless", "--no-remote", "--profile", resolve(output, "firefox-profile"),
    "--screenshot", resolve(output, "sheet.png"), "--window-size", `${width * 5},${height * 6}`,
    pathToFileURL(sheet).href], { timeout: 60000, stdio: ["ignore", "ignore", "pipe"] });
  execFileSync("convert", [resolve(output, "sheet.png"), "-crop", `${width}x${height}`, "+repage",
    resolve(output, "frames/frame-%02d.png")], { timeout: 30000 });

  const frames = Array.from({ length: count }, (_, i) => `frames/frame-${String(i).padStart(2, "0")}.png`);
  for (const file of frames) {
    const png = readFileSync(resolve(output, file));
    if (png.readUInt32BE(16) !== width || png.readUInt32BE(20) !== height) {
      throw new Error(`Unexpected frame dimensions: ${file}`);
    }
  }
  writeFileSync(resolve(output, "manifest.json"), JSON.stringify({ revision, width, height, fps, frames }, null, 2) + "\n");
  console.log(`Prepared ${count} lossless Canvas frames in ${output}`);
}

try { main(); } catch (error) {
  console.error(`Pixel preparation failed: ${error.stderr?.toString().trim() || error.message}`);
  process.exitCode = 1;
}
