import { test, expect } from "@playwright/test";
import type { Event, Matchup } from "../src/tournaments";

function fixture(): Event {
  const pair: Matchup = {
    a: 0,
    b: 1,
    total: 60,
    a_wins: 28,
    b_wins: 20,
    draws: 8,
    a_technical_wins: 1,
    b_technical_wins: 0,
    pending: 1,
    faults: 1,
    unscored: 1,
  };
  return {
    id: "mixed",
    label: "mixed",
    name: "混合赛况",
    schema: "arena-monitor-v1",
    error: null,
    scanned_unix_ms: 1,
    total: 61,
    completed: 58,
    counts: { completed: 58, pending: 2, corrupt: 1 },
    matched: 61,
    offset: 0,
    limit: 50,
    entrants: ["同名选手 <A>", "同名选手 <A>", "新选手", "未排赛选手"].map(
      (name, id) => ({
        id,
        registered_id: `bot-${id}`,
        name,
        version: `v${id + 1}`,
      }),
    ),
    standings: [0, 1, 2, 3].map((entrant, i) => ({
      entrant,
      rank: i + 1,
      rating: 1500 - i * 20,
      wins: 0,
      draws: 0,
      losses: 0,
      forfeit_wins: 0,
      forfeit_losses: 0,
      unscored: 0,
    })),
    matchups: [
      pair,
      {
        ...pair,
        a: 0,
        b: 2,
        total: 1,
        a_wins: 0,
        b_wins: 0,
        draws: 0,
        a_technical_wins: 0,
        pending: 1,
        faults: 0,
        unscored: 0,
      },
    ],
    matches: Array.from({ length: 61 }, (_, index) => ({
      index,
      map: index % 2,
      map_hash: "fixture",
      enl: index % 2 ? 1 : 0,
      res: index === 60 ? 2 : index % 2 ? 0 : 1,
      state: "pending",
      attempts: [],
    })),
  };
}

test("overview matrix uses full totals, mirrors results and drills into paginated pairs", async ({
  page,
}) => {
  const data = fixture();
  const queries: URLSearchParams[] = [];
  await page.route("**/api/events", (route) =>
    route.fulfill({
      json: {
        schema: "arena-catalog-v1",
        scanned_unix_ms: 1,
        errors: [],
        events: [
          { id: "mixed", label: "mixed", error: null },
          { id: "unready", label: "unready", error: "计划未发布" },
        ],
      },
    }),
  );
  await page.route("**/api/events/*", (route) => {
    const url = new URL(route.request().url());
    if (url.pathname.endsWith("/unready"))
      return route.fulfill({
        json: { id: "unready", label: "unready", error: "计划未发布" },
      });
    const q = url.searchParams;
    queries.push(q);
    const rows = data.matches.filter((m) =>
      ["entrant", "opponent"].every(
        (key) => !q.get(key) || [m.enl, m.res].includes(Number(q.get(key))),
      ),
    );
    const offset = Number(q.get("offset") || 0),
      limit = Number(q.get("limit") || 50);
    return route.fulfill({
      json: {
        ...data,
        offset,
        limit,
        matched: rows.length,
        matches: rows.slice(offset, offset + limit),
      },
    });
  });
  await page.goto(`/#token=${"a".repeat(32)}`);
  const overview = page.getByRole("tab", { name: "赛事概览", exact: true });
  const details = page.getByRole("tab", {
    name: "比赛详情与回放",
    exact: true,
  });
  await expect(overview).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#panel-matches")).toBeHidden();
  await expect(page.locator("#event-standings tbody tr")).toHaveCount(4);
  const cell = page.locator('[data-pair="0:1"]');
  for (const label of [
    "28 / 8 / 20",
    "共 60 场",
    "1技术胜",
    "1未决",
    "1异常",
    "1无评分",
  ])
    await expect(cell).toContainText(label);
  await expect(page.locator('[data-pair="1:0"]')).toContainText("20 / 8 / 28");
  await expect(page.locator('[data-pair="1:0"]')).toContainText("1技术负");
  expect(
    await cell.evaluate(
      (el) => getComputedStyle(el, "::after").backgroundImage,
    ),
  ).toContain("linear-gradient");
  await expect(page.locator(".matchup-self")).toHaveCount(4);
  await expect(page.locator(".matchup-empty")).toHaveCount(8);
  await cell.focus();
  await page.evaluate(
    () => ((window as any).focusedCell = document.activeElement),
  );
  await page.getByRole("button", { name: "立即刷新" }).click();
  await expect.poll(() => queries.length).toBeGreaterThan(1);
  expect(await cell.evaluate((el) => el === (window as any).focusedCell)).toBe(
    true,
  );
  await cell.click();
  await expect(details).toHaveAttribute("aria-selected", "true");
  await expect(page.getByLabel("对决选手", { exact: true })).toHaveValue("0");
  await expect(page.getByLabel("对决对手", { exact: true })).toHaveValue("1");
  await expect(page.locator("#matches-page")).toContainText("60 场匹配");
  await page.getByRole("button", { name: "下一页", exact: true }).click();
  await expect(page.locator("#match-list .match-row")).toHaveCount(10);
  await expect(page.locator("#matches-page")).toContainText("第 2 页");
  await page.getByLabel("选手或版本", { exact: true }).fill("尚未应用");
  const response = page.waitForResponse((r) =>
    r.url().includes("/api/events/mixed?"),
  );
  await page.getByRole("button", { name: "立即刷新" }).click();
  await response;
  expect(queries.at(-1)!.get("q")).toBe("");
  await overview.click();
  await expect(cell).toContainText("共 60 场");
  await cell.click();
  await expect(page.getByLabel("选手或版本", { exact: true })).toHaveValue("");
  await expect(page.locator("#matches-page")).toContainText("第 1 页");
  await page.getByRole("button", { name: "清除筛选" }).click();
  await expect(page.locator("#matches-page")).toContainText("61 场匹配");
  await details.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(overview).toBeFocused();
  await expect(overview).toHaveAttribute("aria-selected", "true");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.locator("#tournament-panel").evaluate((el) => el.scrollWidth),
  ).toBeLessThanOrEqual(390);
  expect(
    await page
      .locator(".matchup-scroll")
      .evaluate((el) => el.scrollWidth > el.clientWidth),
  ).toBe(true);
  await page
    .locator(".matchup-scroll")
    .evaluate((el) => (el.scrollLeft = el.scrollWidth));
  await page.screenshot({
    path: "test-results/tournament-overview-mobile.png",
  });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.locator("#tournament-panel").evaluate((el) => (el.scrollTop = 0));
  await page.screenshot({
    path: "test-results/tournament-overview-desktop.png",
  });
  await page.getByLabel("赛事选择").selectOption("unready");
  await expect(page.locator("#event-warning")).toContainText("计划未发布");
  await expect(page.locator("[data-pair]")).toHaveCount(0);
  await expect(page.locator("#match-list .match-row")).toHaveCount(0);
  await page.getByLabel("赛事选择").selectOption("mixed");
  await expect(cell).toBeVisible();
  await expect(overview).toHaveAttribute("aria-selected", "true");
});
