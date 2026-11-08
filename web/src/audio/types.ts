export type ConnectionStatus = "connecting" | "live" | "paused" | "offline";

export interface AudioStats {
  sampleRate: number;
  channels: number;
  bufferedSeconds: number;
  packetsReceived: number;
  bytesReceived: number;
}
