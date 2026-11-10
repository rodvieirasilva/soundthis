import { describe, it, expect, vi, beforeEach } from "vitest";
import { AudioWebSocketClient } from "./ws";

class MockWebSocket {
  static OPEN = 1;
  static CONNECTING = 0;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState = MockWebSocket.CONNECTING;
  binaryType = "";
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: ((err: unknown) => void) | null = null;

  send = vi.fn();
  close = vi.fn().mockImplementation(() => {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.();
  });

  constructor() {
    setTimeout(() => {
      this.readyState = MockWebSocket.OPEN;
      this.onopen?.();
    }, 10);
  }
}

describe("AudioWebSocketClient", () => {
  let callbacks: {
    onAudioData: ReturnType<typeof vi.fn>;
    onOpen: ReturnType<typeof vi.fn>;
    onClose: ReturnType<typeof vi.fn>;
    onError: ReturnType<typeof vi.fn>;
    onStatusMessage: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    (global as unknown as { WebSocket: unknown }).WebSocket = MockWebSocket;

    callbacks = {
      onAudioData: vi.fn(),
      onOpen: vi.fn(),
      onClose: vi.fn(),
      onError: vi.fn(),
      onStatusMessage: vi.fn(),
    };
  });

  it("should connect and handle messages", async () => {
    const client = new AudioWebSocketClient(callbacks);
    client.connect();

    // Give time to open
    await new Promise((r) => setTimeout(r, 20));
    expect(callbacks.onOpen).toHaveBeenCalled();

    client.sendClientName("My Phone");

    // Receive binary audio data
    const ws = (client as unknown as { ws: MockWebSocket }).ws;
    const arrayBuffer = new ArrayBuffer(16);
    ws.onmessage?.({ data: arrayBuffer });
    expect(callbacks.onAudioData).toHaveBeenCalledWith(arrayBuffer);

    // Receive JSON status message
    ws.onmessage?.({ data: JSON.stringify({ enabled: true }) });
    expect(callbacks.onStatusMessage).toHaveBeenCalledWith({ enabled: true });

    // Disconnect cleanly
    client.disconnect();
    expect(callbacks.onClose).toHaveBeenCalled();
  });

  it("should send playback state message when toggling playback", async () => {
    const client = new AudioWebSocketClient(callbacks);
    client.connect(false);

    await new Promise((r) => setTimeout(r, 20));
    const ws = (client as unknown as { ws: MockWebSocket }).ws;

    client.setPlaybackState(true);
    expect(ws.send).toHaveBeenCalledWith(JSON.stringify({ type: "playback", playing: true }));

    client.setPlaybackState(false);
    expect(ws.send).toHaveBeenCalledWith(JSON.stringify({ type: "playback", playing: false }));

    client.disconnect();
  });

  it("should handle error event and schedule reconnect", () => {
    vi.useFakeTimers();
    const client = new AudioWebSocketClient(callbacks);
    client.connect();

    const ws = (client as unknown as { ws: MockWebSocket }).ws;
    const errorEvent = new Event("error");
    ws.onerror?.(errorEvent);
    expect(callbacks.onError).toHaveBeenCalled();

    // Trigger close to trigger reconnect
    ws.onclose?.();
    expect(callbacks.onClose).toHaveBeenCalled();

    vi.advanceTimersByTime(2000);
    client.disconnect();
    vi.useRealTimers();
  });
});
