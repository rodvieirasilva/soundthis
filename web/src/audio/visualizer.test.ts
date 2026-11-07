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
      scale: vi.fn(),
      resetTransform: vi.fn(),
    };

    canvas = {
      getContext: vi.fn().mockReturnValue(mockCtx),
      getBoundingClientRect: vi.fn().mockReturnValue({ width: 300, height: 100 }),
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
      frequencyBinCount: 32,
      getByteFrequencyData: vi.fn().mockImplementation((arr: Uint8Array) => {
        arr.fill(128);
      }),
    } as unknown as AnalyserNode;

    visualizer.setAnalyser(mockAnalyser);
    visualizer.setLive(true);
    visualizer.setLive(false);
    visualizer.setAnalyser(null);
  });
});
