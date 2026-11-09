import { defineConfig } from "@playwright/test";
import { execSync } from "node:child_process";

function findChromium(): string {
  if (process.env.CHROMIUM_BIN) return process.env.CHROMIUM_BIN;
  for (const bin of ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"]) {
    try {
      execSync(`which ${bin}`, { stdio: "ignore" });
      return bin;
    } catch {
      // Continue search
    }
  }
  return "";
}

const chromiumPath = findChromium();

export default defineConfig({
  testDir: "./e2e",
  timeout: 35000,
  retries: 0,
  workers: 1,
  use: {
    headless: true,
    launchOptions: {
      executablePath: chromiumPath || undefined,
      args: [
        "--no-sandbox",
        "--disable-setuid-sandbox",
        "--disable-gpu",
        "--disable-dev-shm-usage",
        "--autoplay-policy=no-user-gesture-required",
      ],
    },
  },
});
