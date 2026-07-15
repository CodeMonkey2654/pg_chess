import { test, expect } from "@playwright/test";

test("dashboard shows health and navigation", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("text=Dashboard").first()).toBeVisible();
  await expect(page.locator("text=Games").first()).toBeVisible();
  await expect(page.locator("text=Explorer").first()).toBeVisible();
  await expect(page.locator("text=Benchmarks").first()).toBeVisible();
});

test("benchmarks page runs suite", async ({ page }) => {
  await page.goto("/");
  await page.locator("button", { hasText: "Benchmarks" }).click();
  await page.locator("button", { hasText: "Run benchmark suite" }).click();
  await expect(page.locator("table.bench-table tbody tr").first()).toBeVisible({
    timeout: 30_000,
  });
});
