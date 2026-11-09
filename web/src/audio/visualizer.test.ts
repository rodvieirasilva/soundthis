import { describe, it, expect, vi, beforeEach } from "vitest";
import { CanvasVisualizer } from "./visualizer";

describe("CanvasVisualizer", () => {
  let canvas: HTMLCanvasElement;
  let mockCtx: unknown;

  beforeEach(() => {
    mockCtx = {
      clearRect: vi.fn(),
      createLinearGradient: vi.fn().mockReturnValue({
        addColorStop: vi.fn(),
      }),
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

    canvas = {
      getContext: vi.fn().mockReturnValue(mockCtx),
      getBoundingClientRect: vi.fn().mockReturnValue({ width: 300, height: 100 }),
      parentElement: {
        getBoundingClientRect: vi.fn().mockReturnValue({ width: 300, height: 100 }),
      },
      width: 300,
      height: 100,
    } as unknown as HTMLCanvasElement;
  });

  it("should fail when 2d context is not supported", () => {
    const badCanvas = {
      getContext: vi.fn().mockReturnValue(null),
    } as unknown as HTMLCanvasElement;

    expect(() => new CanvasVisualizer(badCanvas)).toThrow("Unable to obtain 2D canvas context");
  });

  it("should start and stop rendering loop", () => {
    const visualizer = new CanvasVisualizer(canvas);
    visualizer.start();
    // starting again shouldn't duplicate
    visualizer.start();

    visualizer.stop();
    visualizer.stop();
  });

  it("should handle analyser data and live toggle", () => {
    const visualizer = new CanvasVisualizer(canvas);

    const mockAnalyser = {
      frequencyBinCount: 128,
      getByteFrequencyData: vi.fn().mockImplementation((arr: Uint8Array) => {
        arr.fill(128);
      }),
    } as unknown as AnalyserNode;

    visualizer.setAnalyser(mockAnalyser);
    expect(visualizer.isLiveActive()).toBe(false);

    visualizer.setLive(true);
    expect(visualizer.isLiveActive()).toBe(true);

    visualizer.setLive(false);
    expect(visualizer.isLiveActive()).toBe(false);

    visualizer.setAnalyser(null);
  });

  it("should switch modes and render bars, wave, line, and off", () => {
    const visualizer = new CanvasVisualizer(canvas);
    const mockAnalyser = {
      frequencyBinCount: 256,
      getByteFrequencyData: vi.fn().mockImplementation((arr: Uint8Array) => {
        for (let i = 0; i < arr.length; i++) {
          arr[i] = (i * 3) % 255;
        }
      }),
    } as unknown as AnalyserNode;

    visualizer.setAnalyser(mockAnalyser);
    visualizer.setLive(true);

    // Default mode is bars
    expect(visualizer.getMode()).toBe("bars");
    (visualizer as unknown as { draw: () => void }).draw();

    // Mode: wave
    visualizer.setMode("wave");
    expect(visualizer.getMode()).toBe("wave");
    (visualizer as unknown as { draw: () => void }).draw();

    // Mode: line
    visualizer.setMode("line");
    expect(visualizer.getMode()).toBe("line");
    (visualizer as unknown as { draw: () => void }).draw();

    // Mode: off
    visualizer.setMode("off");
    expect(visualizer.getMode()).toBe("off");
    visualizer.start(); // calling start in off mode should clear and return
  });

  it("should handle resize gracefully when rect is zero", () => {
    const zeroCanvas = {
      getContext: vi.fn().mockReturnValue(mockCtx),
      getBoundingClientRect: vi.fn().mockReturnValue({ width: 0, height: 0 }),
      width: 0,
      height: 0,
    } as unknown as HTMLCanvasElement;

    const visualizer = new CanvasVisualizer(zeroCanvas);
    visualizer.handleResize();
    (visualizer as unknown as { draw: () => void }).draw();
  });

  it("should reuse pre-allocated array buffer across multiple draw frames to avoid heap churn", () => {
    const visualizer = new CanvasVisualizer(canvas);
    const mockAnalyser = {
      frequencyBinCount: 256,
      getByteFrequencyData: vi.fn(),
    } as unknown as AnalyserNode;

    visualizer.setAnalyser(mockAnalyser);
    visualizer.setLive(true);

    const f1 = (visualizer as unknown as { getFrequencies: (c: number) => number[] }).getFrequencies(32);
    const f2 = (visualizer as unknown as { getFrequencies: (c: number) => number[] }).getFrequencies(32);
    expect(f1).toBe(f2);
  });
});
