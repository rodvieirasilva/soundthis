export type ConnectionStatus = "connecting" | "live" | "paused" | "offline";
export type VisualizerMode = "bars" | "wave" | "line" | "off";

export interface AudioStats {
  sampleRate: number;
  channels: number;
  bufferedSeconds: number;
  packetsReceived: number;
  bytesReceived: number;
}
