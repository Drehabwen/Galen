async (page) => {
  await page.locator(".workbench-rail-primary .workbench-rail-action").nth(2).click();
  await page.locator(".agent-benchmark").waitFor();
  await page.locator(".rehab-import .btn-primary").click();
  await page.locator(".rehab-review").waitFor();
  await page.screenshot({ path: "rust/crates/galen/output/playwright/01-case-imported.png", fullPage: true });
  await page.locator(".rehab-review button").first().click();
  await page.locator(".rehab-eval-button").click();
  await page.locator(".rehab-eval:not(.agent-benchmark)").waitFor();
  const title = await page.locator(".rehab-eval:not(.agent-benchmark) h2").textContent();
  if (!title || !title.includes("5")) throw new Error("golden journey result is missing");
  const openReviews = await page.locator(".rehab-strip>div").nth(2).locator("strong").textContent();
  if (openReviews !== "0") throw new Error(`expected zero open reviews, received ${openReviews}`);
  await page.screenshot({ path: "rust/crates/galen/output/playwright/02-evaluation-passed.png", fullPage: true });
}
