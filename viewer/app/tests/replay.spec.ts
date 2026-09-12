import { test, expect, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
const fixtures = resolve("tests/fixtures");
const native = JSON.parse(
  readFileSync(resolve(fixtures, "normal.json"), "utf8"),
);
async function open(page: Page, file = "normal.iarp") {
  await page.goto("/");
  await page.locator("#file-input").setInputFiles(resolve(fixtures, file));
  await expect(page.locator("#verified")).toHaveText("✓ VERIFIED");
}
async function seek(page: Page, round: number) {
  await page.locator("#jump").fill(String(round));
  await page.locator("#jump").press("Tab");
  await expect(page.locator("#seek")).toHaveValue(String(round));
}

test("standalone replay identity keeps versions and sources distinct and resets for old files", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const Original = window.Worker;
    window.Worker = class extends Original {
      constructor(url: URL | string, options?: WorkerOptions) {
        super(url, options);
        this.addEventListener("message", (e) => {
          if (e.data.replay) (window as any).parsedReplay = e.data.replay;
        });
      }
    };
  });
  await open(page, "identity.iarp");
  expect(await page.evaluate(() => (window as any).parsedReplay)).toEqual(
    JSON.parse(readFileSync(resolve(fixtures, "identity.json"), "utf8")),
  );
  await expect(page.locator("#identity-enl")).toContainText("星际 <astra-bot>");
  await expect(page.locator("#version-enl")).toHaveText("v1");
  await expect(page.locator("#version-res")).toHaveText("v2");
  await page.locator("#identity-details summary").click();
  await expect(page.locator("#identity-details")).toContainText(
    "different-self-report",
  );
  await expect(page.locator("#identity-details")).toContainText("sdk-0.1.0");
  await page.locator('[data-view="res"]').click();
  await seek(page, 80);
  await expect(page.locator("#version-enl")).toHaveText("v1");
  await page.setViewportSize({ width: 650, height: 900 });
  await expect(page.locator("#identity-enl")).toBeVisible();
  await expect(page.locator("#identity-res")).toBeVisible();
  await page
    .locator("#file-input")
    .setInputFiles(resolve(fixtures, "normal.iarp"));
  await expect(page.locator("#identity-enl")).toHaveText("选手未知");
  await expect(page.locator("#version-enl")).toHaveText("版本未知");
  await expect(page.locator("#identity-details")).not.toContainText(
    "different-self-report",
  );
  await page
    .locator("#file-input")
    .setInputFiles(resolve(fixtures, "identity-abnormal.iarp"));
  await expect(page.locator("#identity-enl")).toHaveText("self-reported");
  await expect(page.locator("#version-enl")).toHaveText("own-v3");
  await expect(page.locator("#version-res")).toHaveText("版本未知");
  await expect(page.locator("#resultpill")).toContainText("ABNORMAL");
});

test("browser WASM payload equals the native verifier, and rejects corrupt archives", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const external: string[] = [];
  page.on("request", (r) => {
    if (
      !r.url().startsWith("http://127.0.0.1:4173") &&
      !r.url().startsWith("blob:")
    )
      external.push(r.url());
  });
  await page.addInitScript(() => {
    const Original = window.Worker;
    window.Worker = class extends Original {
      constructor(url: URL | string, options?: WorkerOptions) {
        super(url, options);
        this.addEventListener("message", (e) => {
          if (e.data.replay) (window as any).parsedReplay = e.data.replay;
        });
      }
    };
  });
  await open(page);
  expect(await page.evaluate(() => (window as any).parsedReplay)).toEqual(
    native,
  );
  await page.locator("#btn-last").click();
  for (const [i, fac] of ["enl", "res"].entries())
    expect(await page.locator(`#score-${fac}`).innerText()).toBe(
      (BigInt(native.header.outcome.final_score2[i]) / 2n).toLocaleString(
        "en-US",
      ) + (BigInt(native.header.outcome.final_score2[i]) % 2n ? ".5" : ""),
    );
  await expect(page.locator("#verified")).toHaveAttribute(
    "title",
    native.header.outcome.final_hash,
  );
  await page.locator("#file-input").setInputFiles({
    name: "broken.iarp",
    mimeType: "application/octet-stream",
    buffer: Buffer.from("IARP invalid"),
  });
  await expect(page.locator("#load-status")).toContainText("无法打开回放");
  await expect(page.locator("#seek")).toHaveValue("150");
  await page
    .locator("#file-input")
    .setInputFiles(resolve(fixtures, "abnormal.iarp"));
  await expect(page.locator("#resultpill")).toContainText("ABNORMAL");
  await expect(page.locator("#seek")).toHaveAttribute("max", "37");
  expect(errors).toEqual([]);
  expect(external).toEqual([]);
});

test("inventory hover, exact trajectories, zoom, playback, and visibility", async ({
  page,
}) => {
  await open(page);
  const round = 90;
  await seek(page, round);
  const entry = Object.entries(native.keys.enl[round - 1]).find(([, keys]) =>
    Object.values(keys as any).some((n) => (n as number) > 0),
  )!;
  const id = Number(entry[0]),
    portal = Number(Object.keys(entry[1] as any)[0]);
  await page.locator(`[data-select="${id}"]`).click();
  await expect(page.locator("#su-num")).toHaveText(
    String(native.agents.enl[round - 1].find((a: number[]) => a[0] === id)[3]),
  );
  const key = page.locator(`[data-key="${portal}"]`);
  await key.hover();
  await expect(page.locator(`[data-portal="${portal}"]`)).toHaveClass(
    /key-highlight/,
  );
  await expect(page.locator("#portal-info")).toContainText(`A${id}`);
  await page.locator('[data-portal="' + portal + '"]').focus();
  await expect(page.locator(`[data-select="${id}"]`)).toHaveClass(
    /key-highlight/,
  );
  await expect(page.locator(`[data-agent="${id}"]`)).toHaveClass(
    /key-highlight/,
  );
  await page.locator("#zoom-reset").focus();
  await expect(page.locator(`[data-agent="${id}"]`)).not.toHaveClass(
    /key-highlight/,
  );
  const expectedSegments = native.commands.enl
    .slice(0, round)
    .flatMap((r: any, index: number) =>
      (r[id]?.cmds ?? [])
        .filter((c: any) => c.from && c.to)
        .map((c: any) => ({
          r: index + 1,
          d: `M${[c.from[0], native.header.config.map_height_cm - c.from[1]].join(",")}L${[c.to[0], native.header.config.map_height_cm - c.to[1]].join(",")}`,
        })),
    );
  await expect(page.locator(".trajectory.current")).toHaveAttribute(
    "d",
    expectedSegments
      .filter((s: any) => s.r === round)
      .map((s: any) => s.d)
      .join(""),
  );
  await expect(page.locator(".trajectory.history")).toHaveAttribute(
    "d",
    expectedSegments
      .filter((s: any) => s.r < round)
      .map((s: any) => s.d)
      .join(""),
  );
  const initial = await page.locator("#map").getAttribute("viewBox");
  const markerWidth = (await page.locator(".portal-hp").first().boundingBox())!
    .width;
  await page.locator("#zoom-in").click();
  expect(
    (await page.locator(".portal-hp").first().boundingBox())!.width,
  ).toBeCloseTo(markerWidth, 1);
  expect(await page.locator("#map").getAttribute("viewBox")).not.toBe(initial);
  await page.locator("#zoom-reset").click();
  await expect(page.locator("#map")).toHaveAttribute("viewBox", initial!);
  const box = (await page.locator("#map").boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.4);
  await page.mouse.wheel(0, -100);
  await expect(page.locator("#map")).not.toHaveAttribute("viewBox", initial!);
  await page.locator("#zoom-reset").click();
  await page.locator("#zoom-box").click();
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.3);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.65, {
    steps: 8,
  });
  await page.mouse.up();
  await expect(page.locator("#map")).not.toHaveAttribute("viewBox", initial!);
  await page.locator("#zoom-box").click();
  const beforePan = await page.locator("#map").getAttribute("viewBox");
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.45, box.y + box.height * 0.6, {
    steps: 5,
  });
  await page.mouse.up();
  await expect(page.locator("#map")).not.toHaveAttribute("viewBox", beforePan!);
  await page.locator("#focus-agent").click();
  await expect(page.locator("#zoom-level")).toHaveText("5.0×");
  await page.locator('[data-view="res"]').click();
  for (const row of native.agents.enl[round - 1])
    await expect(page.locator(`[data-agent="${row[0]}"]`)).toHaveCount(0);
  await page.locator('[data-view="public"]').click();
  await expect(page.locator("[data-agent]")).toHaveCount(0);
  await expect(page.locator("[data-key]")).toHaveCount(0);
  await expect(page.locator(".trajectory")).toHaveCount(0);
  await page.locator('[data-view="god"]').click();
  await seek(page, 74);
  const before = await page.locator("[data-select]").count();
  await seek(page, 75);
  expect(await page.locator("[data-select]").count()).toBeGreaterThan(before);
  await page.locator("#btn-play").click();
  await expect
    .poll(async () => Number(await page.locator("#seek").inputValue()))
    .toBeGreaterThan(75);
  await page.locator("#btn-play").click();
  await page.locator("#btn-first").click();
  await expect(page.locator("#seek")).toHaveValue("0");
  await page.locator("#map").focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.locator("#seek")).toHaveValue("1");
  await seek(page, 90);
  await page.locator(`[data-select="${id}"]`).click();
  await page.locator("#zoom-reset").click();
  await page.screenshot({
    path: "test-results/replay-desktop.png",
    fullPage: true,
  });
});

test("file drop, replacement and narrow screen layout", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.screenshot({
    path: "test-results/replay-empty-mobile.png",
    fullPage: true,
  });
  const bytes = Array.from(readFileSync(resolve(fixtures, "normal.iarp")));
  await page.evaluate((bytes) => {
    const dt = new DataTransfer();
    dt.items.add(new File([new Uint8Array(bytes)], "dropped.iarp"));
    document.dispatchEvent(
      new DragEvent("drop", { dataTransfer: dt, bubbles: true }),
    );
  }, bytes);
  await expect(page.locator("#verified")).toHaveText("✓ VERIFIED");
  await seek(page, 90);
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBeLessThanOrEqual(390);
  await page.screenshot({
    path: "test-results/replay-mobile.png",
    fullPage: true,
  });
});

test("multi-move bends, failed movement, zero-round archive and design controls", async ({
  page,
}) => {
  const movement = JSON.parse(
    readFileSync(resolve(fixtures, "movement.json"), "utf8"),
  );
  await open(page, "movement.iarp");
  await seek(page, 2);
  await expect(page.locator(".trajectory.current")).toHaveAttribute(
    "data-segments",
    "2",
  );
  await expect(page.locator(".trajectory.history")).toHaveAttribute(
    "data-segments",
    "2",
  );
  await expect(page.locator("#pos-moved")).toContainText("2.00 m");
  await expect(page.locator("#cmdlist .cmd")).toHaveCount(4);
  await expect(page.locator("#cmdlist .cmd").nth(1)).toContainText("DROP");
  await expect(page.locator("#operation-range")).toHaveAttribute(
    "r",
    String(movement.header.config.portal_operation_range_cm),
  );
  await expect(page.locator(".portal-hp")).toHaveCount(
    movement.header.map.portals.length,
  );
  const original = await page.locator("#d-name").innerText();
  await page.locator("#map").focus();
  await page.keyboard.press("]");
  await expect(page.locator("#d-name")).not.toHaveText(original);
  await page.keyboard.press("[");
  await expect(page.locator("#d-name")).toHaveText(original);
  await page.keyboard.press("r");
  await expect(page.locator('[data-view="res"]')).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.keyboard.press("e");
  await expect(page.locator('[data-view="enl"]')).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.keyboard.press("g");
  await expect(page.locator('[data-view="god"]')).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  const key = page.locator("[data-key]").first();
  const portal = await key.getAttribute("data-key");
  await page.locator(`[data-portal="${portal}"] text`).hover();
  await expect(page.locator(".chip.key-highlight")).not.toHaveCount(0);
  await expect(page.locator(".map-agent.key-highlight")).not.toHaveCount(0);
  await page.locator("#zoom-reset").hover();
  await expect(page.locator(".chip.key-highlight")).toHaveCount(0);
  await page
    .locator("#file-input")
    .setInputFiles(resolve(fixtures, "zero.iarp"));
  await expect(page.locator("#seek")).toHaveAttribute("max", "0");
  await page.locator("#btn-play").click();
  await expect(page.locator("#btn-play")).toHaveAttribute("aria-label", "播放");
  await expect(page.locator("#su-num")).toHaveText(
    String(movement.header.config.initial_supply),
  );
  await expect(page.locator("#score-enl")).toHaveText("0");
});
