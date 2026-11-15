import { chromium } from "@playwright/test";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

function findChromium() {
  if (process.env.CHROMIUM_BIN) return process.env.CHROMIUM_BIN;
  for (const bin of ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"]) {
    if (fs.existsSync(bin)) return bin;
  }
  return undefined;
}

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

const args = process.argv.slice(2);
const port = args[0] || "9697";
const outputPath = args[1] || "/tmp/web_panel.png";
const width = parseInt(args[2] || "400", 10);
const height = parseInt(args[3] || "600", 10);

const wavPath = path.join(os.tmpdir(), `soundthis_music_${process.pid}.wav`);
generateRichWav(wavPath, 8);
const audioPlayer = spawn("paplay", [wavPath], { stdio: "ignore" });

const browser = await chromium.launch({
  headless: true,
  executablePath: findChromium(),
  args: ["--no-sandbox", "--disable-gpu", "--autoplay-policy=no-user-gesture-required"],
});

try {
  const page = await browser.newPage();
  await page.setViewportSize({ width, height });
  await page.goto(`http://127.0.0.1:${port}/`);

  // Start live listening so audio streams and visualizer bars bounce
  try {
    await page.getByTestId("play-btn").click({ timeout: 2000 });
    await page.waitForSelector('[data-testid="status-badge"].live', { timeout: 3000 });
    // Let visualizer render active frequency bars
    await page.waitForTimeout(800);
  } catch {
    await page.waitForTimeout(600);
  }

  await page.screenshot({ path: outputPath });
} finally {
  await browser.close();
  audioPlayer.kill();
  if (fs.existsSync(wavPath)) {
    fs.unlinkSync(wavPath);
  }
}
