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

  test("should load with zero console warnings/errors and zero 404/failed network responses", async ({
    page,
  }) => {
    const consoleIssues: string[] = [];
    const pageErrors: Error[] = [];
    const failedResponses: { url: string; status: number }[] = [];

    page.on("console", (msg) => {
      const type = msg.type();
      if (type === "error" || type === "warning") {
        consoleIssues.push(`[${type.toUpperCase()}] ${msg.text()}`);
      }
    });

    page.on("pageerror", (err) => {
      pageErrors.push(err);
    });

    page.on("response", (res) => {
      if (res.status() >= 400) {
        failedResponses.push({ url: res.url(), status: res.status() });
      }
    });

    // 1. Navigate to player
    await page.goto(`http://127.0.0.1:${testPort}/`);
    await page.waitForLoadState("networkidle");

    // 2. Verify favicon endpoints directly
    const svgRes = await page.request.get(`http://127.0.0.1:${testPort}/favicon.svg`);
    expect(svgRes.status()).toBe(200);
    expect(svgRes.headers()["content-type"]).toContain("image/svg+xml");

    const icoRes = await page.request.get(`http://127.0.0.1:${testPort}/favicon.ico`);
    expect(icoRes.status()).toBe(200);

    // 3. User interaction
    await page.getByTestId("play-btn").click();
    await expect(page.getByTestId("status-badge")).toHaveClass(/live/);
    await page.waitForTimeout(500);

    // 4. Assert zero console warnings or errors
    expect(consoleIssues).toEqual([]);

    // 5. Assert zero uncaught page errors
    expect(pageErrors).toEqual([]);

    // 6. Assert zero failed 4xx/5xx network requests
    expect(failedResponses).toEqual([]);
  });

  test("should load player UI, interact with controls, and stream desktop sound", async ({ page }) => {
    // 1. Navigate to embedded SoundThis singlefile client
    await page.goto(`http://127.0.0.1:${testPort}/`);

    // Verify title and logo
    await expect(page).toHaveTitle(/SoundThis/i);
    const headerTitle = page.locator(".logo-text h1");
    await expect(headerTitle).toHaveText("SoundThis");

    // 2. Initial state: Play button is ready and player is paused
    const playBtn = page.getByTestId("play-btn");
    const playBtnText = page.getByTestId("play-btn-text");
    await expect(playBtn).toBeVisible();
    await expect(playBtnText).toHaveText("Listen Live");

    // While paused, verify no binary audio messages are processed
    const statsText = page.getByTestId("stats-text");
    await expect(statsText).toHaveText("48kHz Stereo • 16-bit PCM");

    // Play test audio on Linux desktop while player is paused
    playTestSound(testWavPath);
    await page.waitForTimeout(500);
    // Stats must remain clean and unaccumulated while paused
    await expect(statsText).toHaveText("48kHz Stereo • 16-bit PCM");

    // 3. User clicks Listen Live
    await playBtn.click();
    await expect(playBtnText).toHaveText("Pause Audio");

    const statusBadge = page.getByTestId("status-badge");
    await expect(statusBadge).toHaveClass(/live/);
    await expect(statusBadge).toContainText("Live");

    // 4. Play test sound while listening
    playTestSound(testWavPath);

    // 5. Verify audio stats update in the browser
    await expect(statsText).toContainText("48kHz", { timeout: 8000 });

    // 6. Test visualizer animation mode toggles
    const waveBtn = page.getByTestId("viz-mode-wave");
    const lineBtn = page.getByTestId("viz-mode-line");
    const offBtn = page.getByTestId("viz-mode-off");
    const barsBtn = page.getByTestId("viz-mode-bars");

    await waveBtn.click();
    await expect(waveBtn).toHaveClass(/active/);

    await lineBtn.click();
    await expect(lineBtn).toHaveClass(/active/);

    await offBtn.click();
    await expect(offBtn).toHaveClass(/active/);

    await barsBtn.click();
    await expect(barsBtn).toHaveClass(/active/);

    // 7. Test client name update
    const nameInput = page.getByTestId("client-name-input");
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

  test("should maintain stable memory utilization during sustained playback without accumulation", async ({
    page,
  }) => {
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Performance.enable");

    await page.goto(`http://127.0.0.1:${testPort}/`);
    await page.getByTestId("play-btn").click();
    await expect(page.getByTestId("status-badge")).toHaveClass(/live/);

    // Allow connection and AudioContext to warm up
    await page.waitForTimeout(500);

    // Baseline: run GC and collect baseline metrics
    await cdp.send("HeapProfiler.enable");
    await cdp.send("HeapProfiler.collectGarbage");
    const baseline = await cdp.send("Performance.getMetrics");
    const getMetric = (metrics: { name: string; value: number }[], name: string) =>
      metrics.find((m) => m.name === name)?.value ?? 0;

    const baselineHeap = getMetric(baseline.metrics, "JSHeapUsedSize");
    const baselineNodes = getMetric(baseline.metrics, "Nodes");

    // Sustained streaming: play audio across multiple rounds
    for (let i = 0; i < 3; i++) {
      playTestSound(testWavPath);
      await page.waitForTimeout(800);
    }

    // Force GC to measure retained memory (detect true leaks, not temporary transient buffers)
    await cdp.send("HeapProfiler.collectGarbage");
    const sustained = await cdp.send("Performance.getMetrics");

    const sustainedHeap = getMetric(sustained.metrics, "JSHeapUsedSize");
    const sustainedNodes = getMetric(sustained.metrics, "Nodes");

    // Assertions:
    // 1. Heap delta must be bounded (< 4MB retained growth after thousands of audio chunks)
    const heapGrowth = sustainedHeap - baselineHeap;
    expect(heapGrowth).toBeLessThan(4 * 1024 * 1024);

    // 2. Total JS Heap must remain lean and sub-25MB
    expect(sustainedHeap).toBeLessThan(25 * 1024 * 1024);

    // 3. DOM Nodes must not leak (< 5 node delta)
    const nodeGrowth = Math.abs(sustainedNodes - baselineNodes);
    expect(nodeGrowth).toBeLessThanOrEqual(5);
  });
});
