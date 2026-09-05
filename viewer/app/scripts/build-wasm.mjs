import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const run = (command, args) =>
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
const installed = join(
  process.env.CARGO_HOME || join(homedir(), ".cargo"),
  "bin",
  process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen",
);
const bindgen = existsSync(installed) ? installed : "wasm-bindgen";
try {
  const version = execFileSync(bindgen, ["--version"], {
    encoding: "utf8",
  }).trim();
  if (version !== "wasm-bindgen 0.2.100") throw new Error(`Found ${version}`);
} catch {
  console.error(
    "Install the matching CLI: cargo install wasm-bindgen-cli --version 0.2.100 --locked",
  );
  process.exit(1);
}
run("cargo", [
  "build",
  "--locked",
  "--release",
  "-p",
  "replay-web",
  "--target",
  "wasm32-unknown-unknown",
]);
run(bindgen, [
  "target/wasm32-unknown-unknown/release/replay_web.wasm",
  "--target",
  "web",
  "--out-dir",
  "viewer/app/src/wasm",
]);
