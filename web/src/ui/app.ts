import { AudioStreamPlayer } from "../audio/player";
import { CanvasVisualizer } from "../audio/visualizer";
import { AudioWebSocketClient } from "../services/ws";
import { ClientConfigStore } from "../services/storage";
import type { ConnectionStatus, VisualizerMode } from "../audio/types";

export class SoundThisApp {
  private player: AudioStreamPlayer;
  private visualizer: CanvasVisualizer;
  private wsClient: AudioWebSocketClient;
  private status: ConnectionStatus = "connecting";

  // Elements
  private statusBadge!: HTMLElement;
  private playBtn!: HTMLButtonElement;
  private playBtnText!: HTMLElement;
  private playIcon!: HTMLElement;
  private pauseIcon!: HTMLElement;
  private volumeSlider!: HTMLInputElement;
  private volumeText!: HTMLElement;
  private clientNameInput!: HTMLInputElement;
  private statsText!: HTMLElement;
  private vizButtons!: NodeListOf<HTMLButtonElement>;

  constructor() {
    const initialVolume = ClientConfigStore.getVolume();
    this.player = new AudioStreamPlayer(initialVolume);

    const canvas = document.getElementById("visualizer") as HTMLCanvasElement;
    this.visualizer = new CanvasVisualizer(canvas);

    this.wsClient = new AudioWebSocketClient({
      onAudioData: (buffer) => this.handleAudioData(buffer),
      onOpen: () => this.handleWSOpen(),
      onClose: () => this.handleWSClose(),
      onError: () => this.handleWSError(),
      onStatusMessage: (msg) => this.handleStatusMessage(msg),
    });
  }

  public init(): void {
    this.bindDOMElements();
    this.bindEvents();

    const clientName = ClientConfigStore.getClientName();
    this.clientNameInput.value = clientName;

    const vol = this.player.getVolume();
    this.volumeSlider.value = String(vol);
    this.volumeText.textContent = `${Math.round(vol * 100)}%`;

    this.visualizer.start();
    const savedMode = ClientConfigStore.getVisualizerMode();
    this.setVisualizerMode(savedMode);

    this.setStatus("connecting");
    this.wsClient.connect(false);
  }

  private bindDOMElements(): void {
    this.statusBadge = document.getElementById("status-badge")!;
    this.playBtn = document.getElementById("play-btn") as HTMLButtonElement;
    this.playBtnText = document.getElementById("play-btn-text")!;
    this.playIcon = document.getElementById("play-icon")!;
    this.pauseIcon = document.getElementById("pause-icon")!;
    this.volumeSlider = document.getElementById("volume-slider") as HTMLInputElement;
    this.volumeText = document.getElementById("volume-text")!;
    this.clientNameInput = document.getElementById("client-name-input") as HTMLInputElement;
    this.statsText = document.getElementById("stats-text")!;
    this.vizButtons = document.querySelectorAll<HTMLButtonElement>(".viz-btn");
  }

  private bindEvents(): void {
    this.playBtn.addEventListener("click", () => this.togglePlayback());

    this.volumeSlider.addEventListener("input", (e) => {
      const target = e.target as HTMLInputElement;
      const val = parseFloat(target.value);
      this.player.setVolume(val);
      ClientConfigStore.setVolume(val);
      this.volumeText.textContent = `${Math.round(val * 100)}%`;
    });

    this.clientNameInput.addEventListener("change", () => {
      const name = this.clientNameInput.value.trim() || "Device";
      ClientConfigStore.setClientName(name);
      this.wsClient.sendClientName(name);
    });

    this.vizButtons.forEach((btn) => {
      btn.addEventListener("click", () => {
        const mode = btn.dataset.mode as VisualizerMode;
        if (mode) {
          this.setVisualizerMode(mode);
        }
      });
    });

    setInterval(() => this.updateStats(), 1000);
  }

  private setVisualizerMode(mode: VisualizerMode): void {
    ClientConfigStore.setVisualizerMode(mode);
    this.visualizer.setMode(mode);
    this.vizButtons?.forEach((btn) => {
      if (btn.dataset.mode === mode) {
        btn.classList.add("active");
      } else {
        btn.classList.remove("active");
      }
    });
  }

  private async togglePlayback(): Promise<void> {
    const isPlaying = this.player.getIsPlaying();
    if (isPlaying) {
      this.player.setPlaying(false);
      this.wsClient.setPlaybackState(false);
      this.visualizer.setLive(false);
      this.updatePlayButton(false);
      if (this.status === "live") {
        this.setStatus("paused");
      }
    } else {
      await this.player.init();
      this.visualizer.setAnalyser(this.player.getAnalyser());
      this.player.setPlaying(true);
      this.wsClient.setPlaybackState(true);
      this.visualizer.setLive(true);
      this.updatePlayButton(true);
      if (this.status === "paused" || this.status === "connecting") {
        this.setStatus("live");
      }
    }
  }

  private updatePlayButton(isPlaying: boolean): void {
    if (isPlaying) {
      this.playBtn.classList.remove("paused");
      this.playIcon.style.display = "none";
      this.pauseIcon.style.display = "inline";
      this.playBtnText.textContent = "Pause Audio";
    } else {
      this.playBtn.classList.add("paused");
      this.playIcon.style.display = "inline";
      this.pauseIcon.style.display = "none";
      this.playBtnText.textContent = "Listen Live";
    }
  }

  private setStatus(status: ConnectionStatus): void {
    this.status = status;
    this.statusBadge.className = `status-badge ${status}`;

    switch (status) {
      case "live":
        this.statusBadge.innerHTML = '<span class="dot">●</span> Live';
        break;
      case "paused":
        this.statusBadge.innerHTML = '<span class="dot">⏸</span> Paused';
        break;
      case "connecting":
        this.statusBadge.innerHTML = '<span class="dot">⟳</span> Connecting...';
        break;
      case "offline":
        this.statusBadge.innerHTML = '<span class="dot">○</span> Offline';
        break;
    }
  }

  private handleAudioData(buffer: ArrayBuffer): void {
    if (!this.player.getIsPlaying()) {
      return;
    }
    this.player.feedPCM(buffer);
    if (this.status !== "live") {
      this.setStatus("live");
    }
    if (!this.visualizer.isLiveActive()) {
      this.visualizer.setAnalyser(this.player.getAnalyser());
      this.visualizer.setLive(true);
    }
  }

  private handleWSOpen(): void {
    const name = ClientConfigStore.getClientName();
    this.wsClient.sendClientName(name);

    if (this.player.getIsPlaying()) {
      this.wsClient.setPlaybackState(true);
      this.setStatus("live");
    } else {
      this.setStatus("paused");
    }
  }

  private handleWSClose(): void {
    this.setStatus("offline");
    this.visualizer.setLive(false);
  }

  private handleWSError(): void {
    this.setStatus("offline");
  }

  private handleStatusMessage(msg: unknown): void {
    if (typeof msg === "object" && msg !== null && "enabled" in msg) {
      const data = msg as { enabled: boolean };
      if (!data.enabled && this.status === "live") {
        this.setStatus("paused");
      }
    }
  }

  private updateStats(): void {
    if (!this.statsText) return;
    const stats = this.player.getStats();
    if (stats.packetsReceived > 0) {
      const mb = (stats.bytesReceived / (1024 * 1024)).toFixed(1);
      const latencyMs = Math.round(stats.bufferedSeconds * 1000);
      this.statsText.textContent = `${stats.sampleRate / 1000}kHz 16-bit • ~${latencyMs}ms delay • ${mb} MB`;
    } else {
      this.statsText.textContent = "48kHz Stereo • 16-bit PCM";
    }
  }
}
