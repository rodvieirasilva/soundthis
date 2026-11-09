import { test, expect } from "@playwright/test";
import { spawn, execSync, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

function resolveSoundThisBin(): string {
  const candidates = [
    path.resolve(process.cwd(), "../target/release/soundthis"),
    path.resolve(process.cwd(), "../target/debug/soundthis"),
    "soundthis",
  ];
  for (const bin of candidates) {
    if (fs.existsSync(bin)) return bin;
  }
  return "soundthis";
}

function generateTestWav(filePath: string, durationSec = 2, freq = 440, sampleRate = 48000): void {
  const numSamples = durationSec * sampleRate;
  const dataSize = numSamples * 2 * 2;
  const headerSize = 44;
  const buffer = Buffer.alloc(headerSize + dataSize);

  // RIFF header
  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(36 + dataSize, 4);
  buffer.write("WAVE", 8);

  // fmt subchunk
  buffer.write("fmt ", 12);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20);
  buffer.writeUInt16LE(2, 22);
  buffer.writeUInt32LE(sampleRate, 24);
  buffer.writeUInt32LE(sampleRate * 4, 28);
  buffer.writeUInt16LE(4, 32);
  buffer.writeUInt16LE(16, 34);

  // data subchunk
  buffer.write("data", 36);
  buffer.writeUInt32LE(dataSize, 40);

  let offset = 44;
  for (let i = 0; i < numSamples; i++) {
    const val = Math.round(Math.sin((2 * Math.PI * freq * i) / sampleRate) * 18000);
    buffer.writeInt16LE(val, offset);
    buffer.writeInt16LE(val, offset + 2);
    offset += 4;
  }

  fs.writeFileSync(filePath, buffer);
}

function playTestSound(filePath: string): void {
  try {
    execSync(`paplay "${filePath}" 2>/dev/null || aplay "${filePath}" 2>/dev/null || true`, {
      timeout: 5000,
      stdio: "ignore",
    });
  } catch {
    // Ignore if audio sink not present
  }
}

async function startSoundThisServer(port: number): Promise<ChildProcess> {
  const bin = resolveSoundThisBin();
  const child = spawn(bin, ["serve", "--port", String(port)], {
    stdio: ["ignore", "pipe", "pipe"],
  });

  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/health`);
      if (res.ok) return child;
    } catch {
      // Retry
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  child.kill();
  throw new Error(`SoundThis server failed to start on port ${port}`);
}

test.describe("Browser E2E SoundThis Integration", () => {
  const testPort = 59877;
  let serverProcess: ChildProcess | null = null;
  const testWavPath = path.join(os.tmpdir(), `soundthis_e2e_${Date.now()}.wav`);

  test.beforeAll(async () => {
    generateTestWav(testWavPath, 2, 440);
    serverProcess = await startSoundThisServer(testPort);
  });

  test.afterAll(async () => {
    if (serverProcess) {
      serverProcess.kill();
      serverProcess = null;
    }
    if (fs.existsSync(testWavPath)) {
      try {
        fs.unlinkSync(testWavPath);
      } catch {
        // Ignore
      }
    }
  });

  test("should load player UI, interact with controls, and stream desktop sound", async ({ page }) => {
    // 1. Navigate to embedded SoundThis singlefile client
    await page.goto(`http://127.0.0.1:${testPort}/`);

    // Verify title and logo
    await expect(page).toHaveTitle(/SoundThis/i);
    const headerTitle = page.locator(".logo-text h1");
    await expect(headerTitle).toHaveText("SoundThis");

    // 2. Initial state: Play button is ready
    const playBtn = page.locator("#play-btn");
    const playBtnText = page.locator("#play-btn-text");
    await expect(playBtn).toBeVisible();
    await expect(playBtnText).toHaveText("Listen Live");

    // 3. User clicks Listen Live
    await playBtn.click();
    await expect(playBtnText).toHaveText("Pause Audio");

    const statusBadge = page.locator("#status-badge");
    await expect(statusBadge).toHaveClass(/live/);
    await expect(statusBadge).toContainText("Live");

    // 4. Play a test sound via Linux audio command tool
    playTestSound(testWavPath);

    // 5. Verify audio stats update in the browser
    const statsText = page.locator("#stats-text");
    await expect(statsText).toContainText("48kHz", { timeout: 8000 });

    // 6. Test visualizer animation mode toggles
    const waveBtn = page.locator('.viz-btn[data-mode="wave"]');
    const lineBtn = page.locator('.viz-btn[data-mode="line"]');
    const offBtn = page.locator('.viz-btn[data-mode="off"]');
    const barsBtn = page.locator('.viz-btn[data-mode="bars"]');

    await waveBtn.click();
    await expect(waveBtn).toHaveClass(/active/);

    await lineBtn.click();
    await expect(lineBtn).toHaveClass(/active/);

    await offBtn.click();
    await expect(offBtn).toHaveClass(/active/);

    await barsBtn.click();
    await expect(barsBtn).toHaveClass(/active/);

    // 7. Test client name update
    const nameInput = page.locator("#client-name-input");
    await nameInput.fill("Automated Chromium Test");
    await nameInput.press("Enter");
    await page.waitForTimeout(100);

    // 8. Test audio disconnection and auto-reconnection
    // Kill the server to simulate connection drop
    if (serverProcess) {
      serverProcess.kill();
      serverProcess = null;
    }

    // Browser detects close and displays Offline
    await expect(statusBadge).toHaveClass(/offline/, { timeout: 8000 });
    await expect(statusBadge).toContainText("Offline");

    // Restart server: browser should auto-reconnect
    serverProcess = await startSoundThisServer(testPort);
    await expect(statusBadge).toHaveClass(/live/, { timeout: 8000 });
    await expect(statusBadge).toContainText("Live");
  });
});
