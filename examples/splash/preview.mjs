// Preview only: production devctl will render embedded frames in Rust.
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const revision = "a98915a9fa0d0d74c8a2094935a02de4425e9b1d";

async function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      seconds: { type: "string", default: "2" },
      mono: { type: "boolean", default: false },
    },
  });
  if (positionals.length !== 1) {
    throw new Error(
      "Usage: node examples/splash/preview.mjs <ascii-checkout> [--seconds 6] [--mono]",
    );
  }
  const seconds = Number(values.seconds);
  if (!Number.isFinite(seconds) || seconds <= 0 || seconds > 30) {
    throw new Error("--seconds must be greater than 0 and at most 30.");
  }
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    console.log("Preview skipped: input and output must both be terminals.");
    return;
  }
  if (process.stdout.columns < 40 || process.stdout.rows < 12) {
    console.log("Preview skipped: terminal must be at least 40 columns by 12 rows.");
    return;
  }

  const upstream = resolve(positionals[0]);
  const actual = execFileSync("git", ["-C", upstream, "rev-parse", "HEAD"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  if (actual !== revision) {
    throw new Error(`Preview requires ascii revision ${revision}; found ${actual}.`);
  }
  const [piece, { play }] = await Promise.all([
    import(pathToFileURL(resolve(upstream, "src/pieces/black-prism.ts")).href),
    import(pathToFileURL(resolve(upstream, "src/terminal.ts")).href),
  ]);

  // The upstream preview treats any key as skip and Ctrl-C as interruption.
  const result = await play(piece, { seconds, fps: 15, mono: values.mono });
  if (result.interrupted) {
    process.exitCode = 130;
  } else {
    console.log("Preview finished. devctl TUI has not been entered.");
  }
}

main().catch((error) => {
  console.error(`Preview failed: ${error.message}`);
  process.exitCode = 1;
});
