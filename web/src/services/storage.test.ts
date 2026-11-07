import { describe, it, expect, beforeEach } from "vitest";
import { ClientConfigStore } from "./storage";

describe("ClientConfigStore", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("should return a fallback device name when no name is saved", () => {
    const name = ClientConfigStore.getClientName();
    expect(name).toBeTruthy();
    expect(typeof name).toBe("string");
  });

  it("should save and retrieve client name", () => {
    ClientConfigStore.setClientName("Living Room Speaker");
    expect(ClientConfigStore.getClientName()).toBe("Living Room Speaker");
  });

  it("should trim client name and truncate to 40 characters", () => {
    ClientConfigStore.setClientName("   Extremely Long Device Name That Exceeds The Maximum Limit Of Characters Allowed By The Server   ");
    const name = ClientConfigStore.getClientName();
    expect(name.length).toBeLessThanOrEqual(40);
    expect(name.startsWith("Extremely")).toBe(true);
  });

  it("should default volume to 1.0", () => {
    expect(ClientConfigStore.getVolume()).toBe(1.0);
  });

  it("should store and retrieve volume correctly", () => {
    ClientConfigStore.setVolume(0.42);
    expect(ClientConfigStore.getVolume()).toBeCloseTo(0.42);
  });

  it("should ignore invalid stored volume values", () => {
    localStorage.setItem("soundthis_volume", "invalid");
    expect(ClientConfigStore.getVolume()).toBe(1.0);

    localStorage.setItem("soundthis_volume", "-5");
    expect(ClientConfigStore.getVolume()).toBe(1.0);

    localStorage.setItem("soundthis_volume", "2.5");
    expect(ClientConfigStore.getVolume()).toBe(1.0);
  });
});
