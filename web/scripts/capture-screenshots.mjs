import { chromium } from "@playwright/test";
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

function generateRichWav(filePath, durationSec = 10, sampleRate = 48000) {
  const numSamples = durationSec * sampleRate;
  const dataSize = numSamples * 2 * 2;
  const buffer = Buffer.alloc(44 + dataSize);

  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(36 + dataSize, 4);
  buffer.write("WAVE", 8);
  buffer.write("fmt ", 12);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20); // PCM
  buffer.writeUInt16LE(2, 22); // Stereo
  buffer.writeUInt32LE(sampleRate, 24);
  buffer.writeUInt32LE(sampleRate * 4, 28);
  buffer.writeUInt16LE(4, 32);
  buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36);
  buffer.writeUInt32LE(dataSize, 40);

  const freqs = [65, 130, 260, 440, 880, 1760, 3500, 7000, 11000];
  let offset = 44;
  for (let i = 0; i < numSamples; i++) {
    let sample = 0;
    for (let f = 0; f < freqs.length; f++) {
      const weight = 1.0 - (f / freqs.length) * 0.3;
      sample += Math.sin((2 * Math.PI * freqs[f] * i) / sampleRate) * weight;
    }
    const val = Math.round((sample / freqs.length) * 24000);
    buffer.writeInt16LE(val, offset);
    buffer.writeInt16LE(val, offset + 2);
    offset += 4;
  }

  fs.writeFileSync(filePath, buffer);
}

function findChromium() {
  if (process.env.CHROMIUM_BIN) return process.env.CHROMIUM_BIN;
  for (const bin of ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"]) {
    if (fs.existsSync(bin)) return bin;
  }
  return undefined;
}

async function main() {
  const testPort = 59888;
  const rootDir = path.resolve(process.cwd(), "..");
  const binCandidates = [
    path.join(rootDir, "target/release/soundthis"),
    path.join(rootDir, "target/debug/soundthis"),
    "soundthis",
  ];
  const bin = binCandidates.find((c) => fs.existsSync(c)) || "soundthis";

  const wavPath = path.join(os.tmpdir(), "soundthis_rich_test.wav");
  generateRichWav(wavPath, 10);

  console.log(`Starting SoundThis on port ${testPort}...`);
  const server = spawn(bin, ["serve", "--port", String(testPort)], {
    stdio: "ignore",
  });

  // Wait for health check
  let ready = false;
  for (let i = 0; i < 50; i++) {
    try {
      const res = await fetch(`http://127.0.0.1:${testPort}/health`);
      if (res.ok) {
        ready = true;
        break;
      }
    } catch {
      // Retry
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  if (!ready) {
    server.kill();
    throw new Error("Failed to start SoundThis server");
  }

  // Play audio in background
  const audioPlayer = spawn("paplay", [wavPath], { stdio: "ignore" });

  const browser = await chromium.launch({
    headless: true,
    executablePath: findChromium(),
    args: [
      "--no-sandbox",
      "--disable-gpu",
      "--autoplay-policy=no-user-gesture-required",
    ],
  });

  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${testPort}/`);

    // Click listen live using test id
    await page.getByTestId("play-btn").click();
    await page.getByTestId("status-badge").waitFor({ state: "visible" });
    await page.waitForSelector('[data-testid="status-badge"].live', { timeout: 5000 });

    // Wait for audio stream and visualizer to be active
    await page.waitForTimeout(1500);

    // Desktop screenshot
    await page.setViewportSize({ width: 680, height: 600 });
    await page.waitForTimeout(300);
    const desktopPath = path.join(rootDir, "docs/screenshots/web-client.png");
    await page.screenshot({ path: desktopPath });
    console.log(`Saved desktop screenshot: ${desktopPath}`);

    // Mobile screenshot
    await page.setViewportSize({ width: 440, height: 720 });
    await page.waitForTimeout(300);
    const mobilePath = path.join(rootDir, "docs/screenshots/web-mobile.png");
    await page.screenshot({ path: mobilePath });
    console.log(`Saved mobile screenshot: ${mobilePath}`);
  } finally {
    await browser.close();
    audioPlayer.kill();
    server.kill();
    if (fs.existsSync(wavPath)) {
      fs.unlinkSync(wavPath);
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
