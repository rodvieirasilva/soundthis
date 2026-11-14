import { chromium } from "@playwright/test";
import fs from "node:fs";

function findChromium() {
  if (process.env.CHROMIUM_BIN) return process.env.CHROMIUM_BIN;
  for (const bin of ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"]) {
    if (fs.existsSync(bin)) return bin;
  }
  return undefined;
}

const args = process.argv.slice(2);
const port = args[0] || "9697";
const outputPath = args[1] || "/tmp/web_panel.png";
const width = parseInt(args[2] || "400", 10);
const height = parseInt(args[3] || "600", 10);

const browser = await chromium.launch({
  headless: true,
  executablePath: findChromium(),
  args: ["--no-sandbox", "--disable-gpu", "--autoplay-policy=no-user-gesture-required"],
});

try {
  const page = await browser.newPage();
  await page.setViewportSize({ width, height });
  await page.goto(`http://127.0.0.1:${port}/`);
  await page.waitForTimeout(600);
  await page.screenshot({ path: outputPath });
} finally {
  await browser.close();
}
