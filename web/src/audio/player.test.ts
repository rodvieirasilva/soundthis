import { describe, it, expect, vi, beforeEach } from "vitest";
import { AudioStreamPlayer } from "./player";

class MockGainNode {
  gain = {
    setValueAtTime: vi.fn(),
    cancelScheduledValues: vi.fn(),
    setTargetAtTime: vi.fn(),
  };
  connect = vi.fn();
}

class MockAnalyserNode {
  fftSize = 512;
  minDecibels = -85;
  maxDecibels = -25;
  smoothingTimeConstant = 0.75;
  connect = vi.fn();
}

class MockBufferSource {
  buffer: unknown = null;
  connect = vi.fn();
  start = vi.fn();
}

class MockAudioContext {
  state: "suspended" | "running" = "suspended";
  currentTime = 0;
  destination = {};

  resume = vi.fn().mockImplementation(async () => {
    this.state = "running";
  });

  createGain = vi.fn().mockImplementation(() => new MockGainNode());
  createAnalyser = vi.fn().mockImplementation(() => new MockAnalyserNode());
  createBuffer = vi.fn().mockImplementation((channels: number, length: number, sampleRate: number) => ({
    numberOfChannels: channels,
    length,
    sampleRate,
    duration: length / sampleRate,
    getChannelData: vi.fn().mockReturnValue(new Float32Array(length)),
  }));
  createBufferSource = vi.fn().mockImplementation(() => new MockBufferSource());
}

describe("AudioStreamPlayer", () => {
  beforeEach(() => {
    (window as unknown as { AudioContext: unknown }).AudioContext = MockAudioContext;
  });

  it("should initialize with custom volume", () => {
    const player = new AudioStreamPlayer(0.75);
    expect(player.getVolume()).toBe(0.75);
  });

  it("should clamp volume between 0 and 1", () => {
    const player = new AudioStreamPlayer(1.0);
    player.setVolume(1.5);
    expect(player.getVolume()).toBe(1.0);

    player.setVolume(-0.2);
    expect(player.getVolume()).toBe(0.0);
  });

  it("should track playback state", () => {
    const player = new AudioStreamPlayer();
    expect(player.getIsPlaying()).toBe(false);

    player.setPlaying(true);
    expect(player.getIsPlaying()).toBe(true);

    player.setPlaying(false);
    expect(player.getIsPlaying()).toBe(false);
  });

  it("should initialize AudioContext and resume if suspended", async () => {
    const player = new AudioStreamPlayer();
    expect(player.getAnalyser()).toBeNull();

    await player.init();
    expect(player.getAnalyser()).not.toBeNull();

    // Calling init again when already initialized
    await player.init();
    expect(player.getAnalyser()).not.toBeNull();
  });

  it("should feed PCM data and create audio buffer sources", async () => {
    const player = new AudioStreamPlayer();
    await player.init();
    player.setPlaying(true);

    // 4 samples * 2 channels * 2 bytes = 16 bytes
    const buffer = new Int16Array([1000, -1000, 2000, -2000, 3000, -3000, 4000, -4000]).buffer;
    player.feedPCM(buffer);

    const stats = player.getStats();
    expect(stats.packetsReceived).toBe(1);
    expect(stats.bytesReceived).toBe(16);
    expect(stats.sampleRate).toBe(48000);
    expect(stats.channels).toBe(2);
  });

  it("should adjust gain target when volume changes during playback", async () => {
    const player = new AudioStreamPlayer(0.5);
    await player.init();
    player.setPlaying(true);

    player.setVolume(0.8);
    expect(player.getVolume()).toBe(0.8);
  });

  it("should ignore PCM data when not playing", () => {
    const player = new AudioStreamPlayer();
    const buffer = new ArrayBuffer(16);
    player.feedPCM(buffer);

    const stats = player.getStats();
    expect(stats.packetsReceived).toBe(1);
    expect(stats.bytesReceived).toBe(16);
  });
});
