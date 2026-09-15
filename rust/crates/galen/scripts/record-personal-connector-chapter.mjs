import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";

const galenUrl = process.env.GALEN_DEMO_URL ?? "http://127.0.0.1:1420/?e2e=personal";
const rehabGptUrl = process.env.REHABGPT_DEMO_URL ?? "http://127.0.0.1:5175/chat";
const outputRoot = path.resolve("..", "..", "..", "output", "connector-flow-recording");
const videoDir = path.join(outputRoot, "personal-connector-chapter");
const outputFile = path.join(outputRoot, "Galen-个人连接器到研究假设.webm");
const chromiumPath = path.join(
  process.env.LOCALAPPDATA ?? "",
  "ms-playwright",
  "chromium_headless_shell-1243",
  "chrome-headless-shell-win64",
  "chrome-headless-shell.exe",
);

fs.mkdirSync(videoDir, { recursive: true });

const browser = await chromium.launch({ headless: true, executablePath: chromiumPath });
const context = await browser.newContext({
  viewport: { width: 1600, height: 900 },
  recordVideo: { dir: videoDir, size: { width: 1600, height: 900 } },
});

await context.addInitScript(() => {
  const patient = { patientId: "RID-CHILD-014", patientName: "林晓宁" };
  localStorage.setItem("chatbot-state", JSON.stringify({ state: patient, version: 1 }));
  localStorage.setItem("chatbot-agent-state", JSON.stringify({
    state: {
      ...patient,
      patientAge: 13,
      patientSex: "female",
      patientSessionId: "rehabgpt-014",
      branch: "free_chat",
      stepIndex: 0,
      answers: { age: 13, gender: "女" },
      view: "chat",
      hasHistory: true,
      hasDueReminder: false,
      lastAssessmentSummary: "最近两周疼痛评分上升，训练完成率下降。",
      messages: [
        { id: "m1", role: "bot", text: "我已同步你最近两周的训练、疼痛和量表记录。", timestamp: 1788900000000 },
        { id: "m2", role: "user", text: "这周训练完成得不太稳定，疼痛也比前几天明显。", timestamp: 1788900300000 },
        { id: "m3", role: "bot", text: "已记录。你的连续康复数据可在 Galen 中作为研究数据源使用。", timestamp: 1788900600000 },
      ],
      riskResult: null,
    },
    version: 1,
  }));
});

const page = await context.newPage();
const wait = (milliseconds) => page.waitForTimeout(milliseconds);
const point = (x, y) => page.mouse.move(x, y, { steps: 14 });

await page.goto(rehabGptUrl, { waitUntil: "networkidle" });
await wait(1800);
await point(790, 465);
await wait(1400);
await page.screenshot({ path: path.join(videoDir, "01-rehabgpt-personal-record.png") });

await page.goto(galenUrl, { waitUntil: "networkidle" });
await wait(1500);
await page.locator("input[placeholder]").fill("从 RehabGPT 获取 RID-CHILD-014 最近两周的训练、疼痛和量表记录");
await point(790, 778);
await page.locator("input[placeholder]").press("Enter");
await wait(2200);

const bodyAfterDiscovery = await page.locator("body").innerText();
if (!bodyAfterDiscovery.includes("RID-CHILD-014") || !bodyAfterDiscovery.includes("68")) {
  throw new Error("Personal connector discovery did not render the expected RehabID profile.");
}
await page.screenshot({ path: path.join(videoDir, "02-personal-connector-discovered.png") });

const buttons = page.locator("button");
for (let index = 0; index < await buttons.count(); index += 1) {
  const label = await buttons.nth(index).innerText();
  if (/确认|导入|写入/.test(label)) {
    await point(1180, 690);
    await buttons.nth(index).click();
    break;
  }
}
await wait(1800);
await page.screenshot({ path: path.join(videoDir, "03-personal-data-imported.png") });

const piButton = page.getByRole("button", { name: /进入 PI 对话/ });
await piButton.click();
const piInput = page.locator("textarea[placeholder]");
await piInput.waitFor({ state: "visible" });
await piInput.fill("基于 RID-CHILD-014 最近 14 天训练完成率、疼痛评分和量表变化，识别变化模式，提出一个可验证的研究假设并形成分析方案。");
await point(760, 770);
await piInput.press("Enter");
await wait(2400);

const bodyAfterHypothesis = await page.locator("body").innerText();
if (!bodyAfterHypothesis.includes("研究假设")) {
  throw new Error("PI hypothesis response did not render.");
}
await page.screenshot({ path: path.join(videoDir, "04-pi-hypothesis.png") });

const videoPath = await page.video().path();
await context.close();
await browser.close();
fs.copyFileSync(videoPath, outputFile);
console.log(outputFile);
