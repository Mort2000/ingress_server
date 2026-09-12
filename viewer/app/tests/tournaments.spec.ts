import { test, expect } from "@playwright/test";
import { spawn, execFile, type ChildProcess } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { promisify } from "node:util";
import { createInterface } from "node:readline";
const exec = promisify(execFile);
const runner = resolve("../../target/debug/arena-runner");
const bot = resolve("../../target/debug/iteration-client");
const backend = resolve("../host/target/debug/arena-viewer");

test("independent host discovers results, filters versions and opens verified replay without losing list state", async ({
  page,
}) => {
  const root = mkdtempSync(join(tmpdir(), "viewer-browser-"));
  mkdirSync(join(root, "runs"));
  const event = join(root, "runs/event");
  let host: ChildProcess | undefined;
  const start = async (listen: string) => {
    host = spawn(
      backend,
      [
        "serve",
        "--runs",
        join(root, "runs"),
        "--assets",
        resolve("dist"),
        "--listen",
        listen,
        "--poll-ms",
        "100",
      ],
      { stdio: ["ignore", "pipe", "pipe"] },
    );
    return new Promise<string>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("backend startup timeout")),
        10_000,
      );
      createInterface({ input: host!.stdout! }).once("line", (line) => {
        clearTimeout(timer);
        resolve(line);
      });
      host!.once("exit", () => {
        clearTimeout(timer);
        reject(new Error("backend exited before ready"));
      });
    });
  };
  try {
    await exec(runner, [
      "plan",
      "--out",
      event,
      "--client",
      bot,
      "--client",
      bot,
      "--name",
      "星际 <A>",
      "--name",
      "B",
      "--bot-version",
      "v1",
      "--bot-version",
      "v2",
      "--rounds",
      "2",
      "--fast",
    ]);
    const url = await start("127.0.0.1:0");
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(url);
    await expect(page.locator("#tournament-panel")).toBeVisible();
    await expect(page.locator("#event-progress")).toContainText("0 / 2");
    await expect(page.locator("#match-list")).toContainText("星际 <A>");
    await exec(runner, ["resume", "--out", event]);
    await expect(page.locator("#event-progress")).toContainText("2 / 2");
    await page.getByLabel("选手或版本").fill("v2");
    await page.getByLabel("结果筛选").selectOption("draw");
    await page.getByRole("button", { name: "应用筛选" }).click();
    await expect(page.locator("#match-list .match-row")).toHaveCount(2);
    await expect(page.locator("#event-standings")).toContainText("暂定排名");
    await page
      .locator("#match-list")
      .getByRole("button", { name: "打开回放", exact: true })
      .first()
      .click();
    await expect(page.locator("#tournament-panel")).toBeHidden();
    await expect(page.locator("#verified")).toHaveText("✓ VERIFIED");
    await expect(page.locator("#identity-enl")).toHaveText(/星际 <A>|B/);
    await page.getByRole("button", { name: "赛事", exact: true }).click();
    await expect(page.getByLabel("选手或版本")).toHaveValue("v2");
    await expect(page.getByLabel("结果筛选")).toHaveValue("draw");
    await page.getByRole("button", { name: "返回回放" }).click();
    await page
      .locator("#file-input")
      .setInputFiles(resolve("tests/fixtures/normal.iarp"));
    await expect(page.locator("#identity-enl")).toHaveText("选手未知");
    await page.getByRole("button", { name: "赛事", exact: true }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator("#event-progress")).toBeVisible();
    host!.kill();
    await expect(page.locator("#tournament-status")).toContainText("连接中断", {
      timeout: 15_000,
    });
    await expect(page.locator("#match-list .match-row")).toHaveCount(2);
    const renewed = await start(new URL(url).host);
    // Same-origin fragment navigation need not reload the document. A fresh
    // authorization link must still update the tab's token after host restart.
    await page.goto(renewed);
    await expect(page.locator("#tournament-status")).toContainText("已连接");
    expect(new URL(page.url()).hash).toBe("");
    await expect(page.getByLabel("选手或版本")).toHaveValue("v2");
    await page.getByRole("button", { name: "返回回放" }).click();
    await expect(page.locator("#identity-enl")).toHaveText("选手未知");
    expect(errors).toEqual([]);
  } finally {
    if (host && host.exitCode === null && host.signalCode === null) {
      host.kill();
      await new Promise<void>((r) => host!.once("exit", () => r()));
    }
    rmSync(root, { recursive: true, force: true });
  }
});
