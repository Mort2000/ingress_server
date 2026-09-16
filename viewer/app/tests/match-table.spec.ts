import { test, expect } from "@playwright/test";
import type { Event, PlayerStatistics } from "../src/tournaments";

test("player table aligns swapped factions, exact scores, missing telemetry and attention filters", async ({
  page,
}) => {
  const stats: PlayerStatistics = {
    score2: "9007199254740993",
    portals_captured: 10,
    portals_destroyed: 3,
    links_created: 12,
    links_destroyed: 4,
    fields_created: 5,
    fields_destroyed: 2,
    agent_rounds: 40,
    pass_slots: 20,
    not_submitted_slots: 2,
    rejected_slots: 1,
    idle_rounds: 3,
  };
  const data: Event = {
    id: "stats",
    label: "stats",
    error: null,
    scanned_unix_ms: 1,
    total: 3,
    completed: 2,
    matched: 3,
    offset: 0,
    limit: 50,
    counts: { completed: 2, pending: 1 },
    standings: [],
    entrants: [0, 1].map((id) => ({
      id,
      registered_id: `bot${id}`,
      name: "同名 <Bot>",
      version: `v${id}`,
    })),
    matches: [
      {
        index: 0,
        map: 0,
        map_hash: "map",
        enl: 1,
        res: 0,
        state: "completed",
        attempts: [
          {
            number: "0001",
            state: "completed",
            attempt_reason: null,
            error: null,
            replay: false,
            outcome: "res_win",
            rounds: 10,
            score_res: stats.score2,
            score_enl: "0",
            finished_unix_ms: 1,
            statistics: {
              rounds: 10,
              res: stats,
              enl: {
                ...stats,
                score2: "0",
                pass_slots: 0,
                not_submitted_slots: 0,
                rejected_slots: 0,
                idle_rounds: 0,
              },
            },
          },
        ],
      },
      {
        index: 1,
        map: 1,
        map_hash: "map",
        enl: 0,
        res: 1,
        state: "completed",
        attempts: [
          {
            number: "0001",
            state: "completed",
            attempt_reason: null,
            error: null,
            replay: false,
            outcome: "technical_win",
            winner: 1,
            finished_unix_ms: 2,
          },
        ],
      },
      {
        index: 2,
        map: 0,
        map_hash: "map",
        enl: 0,
        res: 1,
        state: "pending",
        attempts: [],
      },
    ],
  };
  let query = new URLSearchParams();
  await page.route("**/api/events", (route) =>
    route.fulfill({
      json: {
        schema: "arena-catalog-v1",
        scanned_unix_ms: 1,
        errors: [],
        events: [{ id: "stats", label: "stats", error: null }],
      },
    }),
  );
  await page.route("**/api/events/*", (route) => {
    query = new URL(route.request().url()).searchParams;
    return route.fulfill({ json: data });
  });
  await page.goto(`/#token=${"a".repeat(32)}`);
  await page.getByRole("tab", { name: "比赛详情与回放" }).click();
  await expect(page.locator(".match-row")).toHaveCount(3);
  const normal = page.locator('[data-match="0"]');
  await expect(normal.locator(".match-player").first()).toContainText(
    "#0 · v0 · RES",
  );
  await expect(normal.locator(".match-score").first()).toContainText(
    "4,503,599,627,370,496.5",
  );
  await expect(normal.locator(".match-idle").first()).toContainText("55.0%");
  await expect(normal.locator(".match-alerts").first()).toContainText("高空过");
  await expect(normal.locator(".match-alerts").first()).toContainText(
    "未提交 2",
  );
  await expect(normal.locator(".match-alerts").last()).toContainText("零分");
  await expect(normal.locator(".match-structures").first()).toHaveText("+10−3");
  const technical = page.locator('[data-match="1"]');
  await expect(technical.locator(".match-player").first()).toContainText(
    "#1 · v1 · RES",
  );
  await expect(technical.locator(".result-badge").first()).toHaveText("技术胜");
  await expect(technical.locator(".match-score").first()).toHaveText("—");
  await expect(page.locator('[data-match="2"] .match-idle').first()).toHaveText(
    "—",
  );
  await expect(
    page.locator('#event-outcome option[value="win"]'),
  ).toHaveJSProperty("disabled", true);
  await page.getByLabel("对决选手", { exact: true }).selectOption("0");
  await page.getByLabel("结果筛选").selectOption("loss");
  await page.getByLabel("关注筛选").selectOption("high_idle");
  await page.getByLabel("比赛排序").selectOption("idle");
  await page.getByRole("button", { name: "应用筛选" }).click();
  await expect.poll(() => query.get("outcome")).toBe("loss");
  expect(query.get("entrant")).toBe("0");
  expect(query.get("attention")).toBe("high_idle");
  expect(query.get("order")).toBe("idle");
  await expect(technical.locator(".match-player").first()).toContainText(
    "#0 · v0 · ENL",
  );
  await normal.locator("summary").click();
  await page.getByRole("button", { name: "立即刷新" }).click();
  await expect(normal.locator("details")).toHaveAttribute("open", "");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page
      .locator("#tournament-panel")
      .evaluate((e) => e.scrollWidth <= e.clientWidth),
  ).toBe(true);
  expect(
    await page
      .locator(".match-table-scroll")
      .evaluate((e) => e.scrollWidth > e.clientWidth),
  ).toBe(true);
});
