export interface WSClientCallbacks {
  onAudioData: (buffer: ArrayBuffer) => void;
  onOpen: () => void;
  onClose: () => void;
  onError: (err: Event) => void;
  onStatusMessage?: (data: unknown) => void;
}

export class AudioWebSocketClient {
  private ws: WebSocket | null = null;
  private callbacks: WSClientCallbacks;
  private reconnectTimer: number | null = null;
  private shouldConnect = true;

  constructor(callbacks: WSClientCallbacks) {
    this.callbacks = callbacks;
  }

  public connect(): void {
    this.shouldConnect = true;
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      return;
    }

    const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${proto}//${window.location.host}/ws`;

    try {
      this.ws = new WebSocket(url);
      this.ws.binaryType = "arraybuffer";

      this.ws.onopen = () => {
        if (this.reconnectTimer) {
          clearTimeout(this.reconnectTimer);
          this.reconnectTimer = null;
        }
        this.callbacks.onOpen();
      };

      this.ws.onmessage = (event: MessageEvent) => {
        if (event.data instanceof ArrayBuffer) {
          this.callbacks.onAudioData(event.data);
        } else if (typeof event.data === "string") {
          try {
            const parsed = JSON.parse(event.data);
            this.callbacks.onStatusMessage?.(parsed);
          } catch {
            // Ignore non-JSON control messages
          }
        }
      };

      this.ws.onclose = () => {
        this.callbacks.onClose();
        if (this.shouldConnect) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = (err) => {
        this.callbacks.onError(err);
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  public sendClientName(name: string): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: "identify", name }));
    }
  }

  public disconnect(): void {
    this.shouldConnect = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer !== null) return;
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      if (this.shouldConnect) {
        this.connect();
      }
    }, 1500);
  }
}
