import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";

const appUrl = process.env.GALEN_DEMO_URL ?? "http://127.0.0.1:1420/?e2e=cohort";
const outputDir = path.resolve("..", "..", "..", "output", "connector-flow-recording", "rehabgpt-connector");
const outputFile = path.resolve("..", "..", "..", "output", "connector-flow-recording", "Galen-RehabGPT-连接器到研究任务.webm");
const chromiumPath = path.join(
  process.env.LOCALAPPDATA ?? "",
  "ms-playwright",
  "chromium_headless_shell-1243",
  "chrome-headless-shell-win64",
  "chrome-headless-shell.exe",
);

fs.mkdirSync(outputDir, { recursive: true });
const browser = await chromium.launch({ headless: true, executablePath: chromiumPath });
const context = await browser.newContext({
  viewport: { width: 1600, height: 900 },
  recordVideo: { dir: outputDir, size: { width: 1600, height: 900 } },
});
const page = await context.newPage();

const beat = async (milliseconds) => page.waitForTimeout(milliseconds);
const point = async (x, y) => page.mouse.move(x, y, { steps: 12 });

await page.goto(appUrl, { waitUntil: "networkidle" });
await point(1120, 250);
await beat(1300);

// First show that Galen has several sources, then keep the narrative in Galen.
await point(1480, 800);
await page.locator("button").nth(19).click();
await beat(2100);
await page.locator("button").nth(19).click();
await beat(500);

const instruction = "从 RehabGPT 获取 RID-CHILD-014 最近两周的训练、疼痛和量表记录";
const input = page.locator("input[placeholder]");
await input.fill(instruction);
await point(725, 780);
await beat(700);
await input.press("Enter");
await beat(2200);

// The connector receipt is the central proof: it is shown before the PI takes over.
await point(1190, 570);
await beat(1700);

const texts = await page.locator("body").innerText();
if (!texts.includes("RehabGPT")) {
  throw new Error("RehabGPT connector preview did not render");
}
await page.screenshot({ path: path.join(outputDir, "connector-preview.png") });

// Confirm the visible import action. The canvas uses one primary button in this state.
const actionButtons = page.locator("button");
const buttonCount = await actionButtons.count();
let imported = false;
for (let index = 0; index < buttonCount; index += 1) {
  const label = await actionButtons.nth(index).innerText();
  if (label.includes("确认") || label.includes("导入") || label.includes("写入")) {
    await point(1180, 690);
    await actionButtons.nth(index).click();
    imported = true;
    break;
  }
}
if (!imported) throw new Error("Connector import action was not found");
await beat(1800);
await page.screenshot({ path: path.join(outputDir, "imported-state.png") });
await page.getByRole("button", { name: "进入 PI 对话 →" }).click();
const piInput = page.locator("textarea[placeholder]");
await piInput.waitFor({ state: "visible" });

// Hand the imported longitudinal record to Galen PI in one natural-language instruction.
await piInput.fill("基于当前 RehabID 时间轴，建立依从性、疼痛与量表变化的纵向分析任务，并交付可验证的研究输出。 ");
await point(725, 780);
await beat(650);
await piInput.press("Enter");
await beat(1400);
await piInput.fill("计划已确认，请开始推进并交付结果。");
await piInput.press("Enter");
await beat(4200);

await page.screenshot({ path: path.join(outputDir, "final-state.png") });
const video = await page.video().path();
await context.close();
await browser.close();
fs.copyFileSync(video, outputFile);
console.log(outputFile);
