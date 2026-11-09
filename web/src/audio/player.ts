import type { AudioStats } from "./types";

export class AudioStreamPlayer {
  private ctx: AudioContext | null = null;
  private gainNode: GainNode | null = null;
  private analyserNode: AnalyserNode | null = null;
  private nextPlayTime = 0;
  private isPlaying = false;
  private volumeLevel = 1.0;
  private sampleRate = 48000;
  private channels = 2;
  private packetsReceived = 0;
  private bytesReceived = 0;

  constructor(initialVolume = 1.0) {
    this.volumeLevel = initialVolume;
  }

  public async init(): Promise<void> {
    if (this.ctx) {
      if (this.ctx.state === "suspended") {
        await this.ctx.resume();
      }
      return;
    }

    const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    this.ctx = new AudioContextClass({
      sampleRate: this.sampleRate,
      latencyHint: "interactive",
    });

    this.gainNode = this.ctx.createGain();
    this.gainNode.gain.setValueAtTime(this.volumeLevel, this.ctx.currentTime);

    this.analyserNode = this.ctx.createAnalyser();
    this.analyserNode.fftSize = 512;
    this.analyserNode.minDecibels = -85;
    this.analyserNode.maxDecibels = -25;
    this.analyserNode.smoothingTimeConstant = 0.75;

    this.gainNode.connect(this.analyserNode);
    this.analyserNode.connect(this.ctx.destination);

    if (this.ctx.state === "suspended") {
      await this.ctx.resume();
    }
    this.nextPlayTime = this.ctx.currentTime + 0.05;
  }

  public getAnalyser(): AnalyserNode | null {
    return this.analyserNode;
  }

  public setPlaying(playing: boolean): void {
    this.isPlaying = playing;
    if (this.gainNode && this.ctx) {
      const now = this.ctx.currentTime;
      this.gainNode.gain.cancelScheduledValues(now);
      this.gainNode.gain.setTargetAtTime(playing ? this.volumeLevel : 0, now, 0.02);
    }
  }

  public getIsPlaying(): boolean {
    return this.isPlaying;
  }

  public setVolume(val: number): void {
    this.volumeLevel = Math.max(0, Math.min(1, val));
    if (this.gainNode && this.ctx && this.isPlaying) {
      this.gainNode.gain.setTargetAtTime(this.volumeLevel, this.ctx.currentTime, 0.02);
    }
  }

  public getVolume(): number {
    return this.volumeLevel;
  }

  public feedPCM(data: ArrayBuffer): void {
    this.packetsReceived++;
    this.bytesReceived += data.byteLength;

    if (!this.isPlaying || !this.ctx || !this.gainNode) {
      return;
    }

    const int16Array = new Int16Array(data);
    const numSamples = Math.floor(int16Array.length / this.channels);
    if (numSamples <= 0) return;

    const audioBuffer = this.ctx.createBuffer(this.channels, numSamples, this.sampleRate);
    const leftChannel = audioBuffer.getChannelData(0);
    const rightChannel = audioBuffer.getChannelData(1);

    for (let i = 0; i < numSamples; i++) {
      const idx = i * 2;
      leftChannel[i] = int16Array[idx] / 32768.0;
      rightChannel[i] = int16Array[idx + 1] / 32768.0;
    }

    const source = this.ctx.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(this.gainNode);
    source.onended = () => {
      source.disconnect();
    };

    const currentTime = this.ctx.currentTime;
    // Keep playback within 40-70ms target latency
    if (this.nextPlayTime < currentTime || this.nextPlayTime > currentTime + 0.2) {
      this.nextPlayTime = currentTime + 0.04;
    }

    source.start(this.nextPlayTime);
    this.nextPlayTime += audioBuffer.duration;
  }

  public getStats(): AudioStats {
    const buffered = this.ctx ? Math.max(0, this.nextPlayTime - this.ctx.currentTime) : 0;
    return {
      sampleRate: this.sampleRate,
      channels: this.channels,
      bufferedSeconds: buffered,
      packetsReceived: this.packetsReceived,
      bytesReceived: this.bytesReceived,
    };
  }
}
