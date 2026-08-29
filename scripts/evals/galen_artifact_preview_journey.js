async (page) => {
  await page.getByText("产物库", { exact: true }).click();
  const ledger = page.locator(".artifact-ledger");
  await ledger.waitFor();
  const count = await ledger.locator(".artifact-ledger-heading strong").textContent();
  if (count !== "1") throw new Error(`expected one registered artifact, received ${count}`);

  await ledger.getByText("output/E09-scoliosis-evidence-brief.md", { exact: true }).click();
  const preview = page.getByTestId("artifact-rendered-preview");
  await preview.waitFor();

  const heading = await preview.locator("h1").textContent();
  if (heading !== "青少年特发性脊柱侧弯证据简报") {
    throw new Error(`rendered heading is missing: ${heading}`);
  }
  const tableRows = await preview.locator("table tbody tr").count();
  if (tableRows !== 2) throw new Error(`expected two rendered result rows, received ${tableRows}`);
  if ((await preview.locator("pre").count()) !== 0) {
    throw new Error("artifact is displayed as source text instead of rendered Markdown");
  }
  const path = await page.locator(".artifact-path").textContent();
  if (path !== "output/E09-scoliosis-evidence-brief.md") throw new Error(`unexpected artifact path: ${path}`);

  await page.screenshot({ path: "rust/crates/galen/output/playwright/03-artifact-rendered-preview.png", fullPage: true });
}
