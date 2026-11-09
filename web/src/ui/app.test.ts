import { describe, it, expect, vi, beforeEach } from "vitest";
import { SoundThisApp } from "./app";

// DOM HTML template matching web/index.html
const HTML_TEMPLATE = `
  <div id="status-badge" class="status-badge connecting"></div>
  <button id="play-btn" class="btn-primary paused">
    <span id="play-icon">Play</span>
    <span id="pause-icon" style="display:none">Pause</span>
    <span id="play-btn-text">Listen Live</span>
  </button>
  <input type="range" id="volume-slider" value="1.0" />
  <span id="volume-text">100%</span>
  <input type="text" id="client-name-input" value="" />
  <span id="stats-text"></span>
  <div class="visualizer-container">
    <canvas id="visualizer" width="300" height="100"></canvas>
    <div class="visualizer-toolbar">
      <div class="viz-btn-group">
        <button type="button" class="viz-btn active" data-mode="bars">Bars</button>
        <button type="button" class="viz-btn" data-mode="wave">Wave</button>
        <button type="button" class="viz-btn" data-mode="line">Line</button>
        <button type="button" class="viz-btn" data-mode="off">Off</button>
      </div>
    </div>
  </div>
`;

class MockAudioContext {
  state = "suspended";
  currentTime = 0;
  destination = {};
  resume = vi.fn().mockImplementation(async () => {
    this.state = "running";
  });
  createGain = vi.fn().mockReturnValue({
    gain: { setValueAtTime: vi.fn(), cancelScheduledValues: vi.fn(), setTargetAtTime: vi.fn() },
    connect: vi.fn(),
  });
  createAnalyser = vi.fn().mockReturnValue({
    fftSize: 64,
    smoothingTimeConstant: 0.8,
    connect: vi.fn(),
  });
  createBuffer = vi.fn().mockReturnValue({
    numberOfChannels: 2,
    length: 100,
    sampleRate: 48000,
    duration: 100 / 48000,
    getChannelData: vi.fn().mockReturnValue(new Float32Array(100)),
  });
  createBufferSource = vi.fn().mockReturnValue({
    buffer: null,
    connect: vi.fn(),
    start: vi.fn(),
  });
}

describe("SoundThisApp", () => {
  beforeEach(() => {
    document.body.innerHTML = HTML_TEMPLATE;
    (window as unknown as { AudioContext: unknown }).AudioContext = MockAudioContext;

    const mockCtx = {
      clearRect: vi.fn(),
      createLinearGradient: vi.fn().mockReturnValue({ addColorStop: vi.fn() }),
      beginPath: vi.fn(),
      roundRect: vi.fn(),
      fill: vi.fn(),
      stroke: vi.fn(),
      moveTo: vi.fn(),
      lineTo: vi.fn(),
      closePath: vi.fn(),
      quadraticCurveTo: vi.fn(),
      scale: vi.fn(),
      resetTransform: vi.fn(),
    };

    HTMLCanvasElement.prototype.getContext = vi.fn().mockReturnValue(mockCtx);
    HTMLCanvasElement.prototype.getBoundingClientRect = vi.fn().mockReturnValue({ width: 300, height: 100 });
  });

  it("should initialize DOM and events correctly", () => {
    const app = new SoundThisApp();
    app.init();

    const nameInput = document.getElementById("client-name-input") as HTMLInputElement;
    expect(nameInput.value).toBeTruthy();

    const playBtn = document.getElementById("play-btn") as HTMLButtonElement;
    expect(playBtn).toBeTruthy();

    const volumeSlider = document.getElementById("volume-slider") as HTMLInputElement;
    expect(volumeSlider.value).toBe("1");
  });

  it("should handle volume change events", () => {
    const app = new SoundThisApp();
    app.init();

    const volumeSlider = document.getElementById("volume-slider") as HTMLInputElement;
    volumeSlider.value = "0.6";
    volumeSlider.dispatchEvent(new Event("input"));

    const volumeText = document.getElementById("volume-text")!;
    expect(volumeText.textContent).toBe("60%");
  });

  it("should handle client name changes", () => {
    const app = new SoundThisApp();
    app.init();

    const nameInput = document.getElementById("client-name-input") as HTMLInputElement;
    nameInput.value = "Office Laptop";
    nameInput.dispatchEvent(new Event("change"));

    expect(localStorage.getItem("soundthis_client_name")).toBe("Office Laptop");
  });

  it("should toggle playback and update UI", async () => {
    const app = new SoundThisApp();
    app.init();

    const playBtn = document.getElementById("play-btn") as HTMLButtonElement;
    const playBtnText = document.getElementById("play-btn-text")!;

    // Start playback
    await (app as unknown as { togglePlayback: () => Promise<void> }).togglePlayback();
    expect(playBtnText.textContent).toBe("Pause Audio");
    expect(playBtn.classList.contains("paused")).toBe(false);

    // Pause playback
    await (app as unknown as { togglePlayback: () => Promise<void> }).togglePlayback();
    expect(playBtnText.textContent).toBe("Listen Live");
    expect(playBtn.classList.contains("paused")).toBe(true);
  });

  it("should handle WebSocket events and status updates", () => {
    const app = new SoundThisApp();
    app.init();

    const statusBadge = document.getElementById("status-badge")!;

    // Open event
    (app as unknown as { handleWSOpen: () => void }).handleWSOpen();
    expect(statusBadge.classList.contains("paused")).toBe(true);

    // Audio data received
    const buffer = new ArrayBuffer(16);
    (app as unknown as { handleAudioData: (b: ArrayBuffer) => void }).handleAudioData(buffer);

    // Status message received
    (app as unknown as { handleStatusMessage: (m: unknown) => void }).handleStatusMessage({ enabled: false });

    // Close event
    (app as unknown as { handleWSClose: () => void }).handleWSClose();
    expect(statusBadge.classList.contains("offline")).toBe(true);

    // Error event
    (app as unknown as { handleWSError: () => void }).handleWSError();
    expect(statusBadge.classList.contains("offline")).toBe(true);
  });

  it("should update stats text", () => {
    const app = new SoundThisApp();
    app.init();

    (app as unknown as { updateStats: () => void }).updateStats();
    const statsText = document.getElementById("stats-text")!;
    expect(statsText.textContent).toContain("48kHz");
  });

  it("should handle visualizer mode button clicks", () => {
    const app = new SoundThisApp();
    app.init();

    const waveBtn = document.querySelector<HTMLButtonElement>('.viz-btn[data-mode="wave"]')!;
    waveBtn.click();
    expect(waveBtn.classList.contains("active")).toBe(true);
    expect(localStorage.getItem("soundthis_viz_mode")).toBe("wave");

    const offBtn = document.querySelector<HTMLButtonElement>('.viz-btn[data-mode="off"]')!;
    offBtn.click();
    expect(offBtn.classList.contains("active")).toBe(true);
    expect(localStorage.getItem("soundthis_viz_mode")).toBe("off");
  });
});
