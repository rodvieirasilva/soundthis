import { describe, it, expect } from "vitest";
import { AudioStreamPlayer } from "./player";

describe("AudioStreamPlayer", () => {
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

  it("should provide initial stats", () => {
    const player = new AudioStreamPlayer();
    const stats = player.getStats();
    expect(stats.sampleRate).toBe(48000);
    expect(stats.channels).toBe(2);
    expect(stats.packetsReceived).toBe(0);
    expect(stats.bytesReceived).toBe(0);
  });
});
