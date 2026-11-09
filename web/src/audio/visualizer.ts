import type { VisualizerMode } from "./types";

export class CanvasVisualizer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private analyser: AnalyserNode | null = null;
  private dataArray: Uint8Array<ArrayBuffer> | null = null;
  private animationId: number | null = null;
  private isLive = false;
  private mode: VisualizerMode = "bars";
  private resizeObserver: ResizeObserver | null = null;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Unable to obtain 2D canvas context");
    }
    this.ctx = context;
    this.handleResize();
    window.addEventListener("resize", () => this.handleResize());

    if (typeof ResizeObserver !== "undefined") {
      this.resizeObserver = new ResizeObserver(() => this.handleResize());
      this.resizeObserver.observe(canvas);
      if (canvas.parentElement) {
        this.resizeObserver.observe(canvas.parentElement);
      }
    }
  }

  public setAnalyser(analyser: AnalyserNode | null): void {
    this.analyser = analyser;
    if (analyser) {
      const bufferLength = analyser.frequencyBinCount;
      this.dataArray = new Uint8Array(bufferLength);
    } else {
      this.dataArray = null;
    }
    this.handleResize();
  }

  public setLive(live: boolean): void {
    this.isLive = live;
    this.handleResize();
  }

  public isLiveActive(): boolean {
    return this.isLive && this.analyser !== null;
  }

  public setMode(mode: VisualizerMode): void {
    this.mode = mode;
    if (mode === "off") {
      this.stop();
      this.clearCanvas();
    } else {
      this.handleResize();
      this.start();
    }
  }

  public getMode(): VisualizerMode {
    return this.mode;
  }

  public start(): void {
    if (this.mode === "off") {
      this.clearCanvas();
      return;
    }
    if (this.animationId !== null) return;
    const render = () => {
      this.draw();
      this.animationId = requestAnimationFrame(render);
    };
    render();
  }

  public stop(): void {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  public handleResize(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      const targetW = Math.round(rect.width * dpr);
      const targetH = Math.round(rect.height * dpr);
      if (this.canvas.width !== targetW || this.canvas.height !== targetH) {
        this.canvas.width = targetW;
        this.canvas.height = targetH;
      }
      this.ctx.resetTransform?.();
      this.ctx.scale(dpr, dpr);
    }
  }

  private clearCanvas(): void {
    const rect = this.canvas.getBoundingClientRect();
    this.ctx.clearRect(0, 0, rect.width, rect.height);
  }

  private getFrequencies(count: number): number[] {
    const fractions = new Array<number>(count).fill(0);
    if (!this.analyser || !this.dataArray || !this.isLive) {
      return fractions;
    }

    this.analyser.getByteFrequencyData(this.dataArray);
    const binCount = this.dataArray.length;
    const nyquist = 24000;
    const fMin = 35;
    const fMax = 15000;

    for (let i = 0; i < count; i++) {
      const fLow = fMin * Math.pow(fMax / fMin, i / count);
      const fHigh = fMin * Math.pow(fMax / fMin, (i + 1) / count);

      const binStart = Math.min(binCount - 1, Math.max(0, Math.floor((fLow / nyquist) * binCount)));
      const binEnd = Math.min(binCount, Math.max(binStart + 1, Math.ceil((fHigh / nyquist) * binCount)));

      let sum = 0;
      let peak = 0;
      let samples = 0;
      for (let b = binStart; b < binEnd; b++) {
        const v = this.dataArray[b] || 0;
        sum += v;
        if (v > peak) peak = v;
        samples++;
      }
      const raw = samples > 0 ? (sum / samples) * 0.6 + peak * 0.4 : 0;

      // Equal-loudness high-frequency tilt
      const tilt = 1.0 + Math.pow(i / (count - 1), 0.75) * 1.5;
      const val = Math.min(255, raw * tilt);

      fractions[i] = Math.max(0, Math.min(1, val / 255.0));
    }

    return fractions;
  }

  private draw(): void {
    if (this.mode === "off") {
      this.clearCanvas();
      return;
    }

    if (this.canvas.width === 0 || this.canvas.height === 0) {
      this.handleResize();
    }

    const rect = this.canvas.getBoundingClientRect();
    const width = rect.width;
    const height = rect.height;

    if (width <= 0 || height <= 0) {
      return;
    }

    this.ctx.clearRect(0, 0, width, height);

    const barCount = 32;
    const fractions = this.getFrequencies(barCount);

    switch (this.mode) {
      case "wave":
        this.drawWave(width, height, fractions);
        break;
      case "line":
        this.drawLine(width, height, fractions);
        break;
      case "bars":
      default:
        this.drawBars(width, height, fractions);
        break;
    }
  }

  private drawBars(width: number, height: number, fractions: number[]): void {
    const barCount = fractions.length;
    const barSpacing = 3;
    const totalSpacing = (barCount - 1) * barSpacing;
    const barWidth = Math.max(2, (width - totalSpacing) / barCount);

    for (let i = 0; i < barCount; i++) {
      const frac = fractions[i] ?? 0;
      const minH = 3;
      const maxH = height - 12;
      const barH = minH + frac * (maxH - minH);
      const x = i * (barWidth + barSpacing);
      const y = height - barH - 4;

      const grad = this.ctx.createLinearGradient(0, y, 0, y + barH);
      if (this.isLive && frac > 0.03) {
        grad.addColorStop(0, "#bb9af7");
        grad.addColorStop(1, "#7aa2f7");
      } else {
        grad.addColorStop(0, "rgba(169, 177, 214, 0.18)");
        grad.addColorStop(1, "rgba(169, 177, 214, 0.06)");
      }

      this.ctx.fillStyle = grad;
      this.ctx.beginPath();
      this.ctx.roundRect(x, y, barWidth, barH, [3, 3, 1, 1]);
      this.ctx.fill();
    }
  }

  private drawWave(width: number, height: number, fractions: number[]): void {
    const barCount = fractions.length;
    const barSpacing = 3;
    const totalSpacing = (barCount - 1) * barSpacing;
    const barWidth = Math.max(2, (width - totalSpacing) / barCount);

    for (let i = 0; i < barCount; i++) {
      const frac = fractions[i] ?? 0;
      const minH = 4;
      const maxH = height - 14;
      const barH = minH + frac * (maxH - minH);
      const x = i * (barWidth + barSpacing);
      const y = (height - barH) / 2;

      const grad = this.ctx.createLinearGradient(0, y, 0, y + barH);
      if (this.isLive && frac > 0.03) {
        grad.addColorStop(0, "#7aa2f7");
        grad.addColorStop(0.5, "#bb9af7");
        grad.addColorStop(1, "#7aa2f7");
      } else {
        grad.addColorStop(0, "rgba(169, 177, 214, 0.18)");
        grad.addColorStop(1, "rgba(169, 177, 214, 0.06)");
      }

      this.ctx.fillStyle = grad;
      this.ctx.beginPath();
      this.ctx.roundRect(x, y, barWidth, barH, 2);
      this.ctx.fill();
    }
  }

  private drawLine(width: number, height: number, fractions: number[]): void {
    const count = fractions.length;
    const step = width / (count - 1);

    // Area fill
    this.ctx.beginPath();
    this.ctx.moveTo(0, height);

    for (let i = 0; i < count; i++) {
      const frac = fractions[i] ?? 0;
      const x = i * step;
      const y = height - 8 - frac * (height - 20);
      if (i === 0) {
        this.ctx.lineTo(x, y);
      } else {
        const prevX = (i - 1) * step;
        const prevFrac = fractions[i - 1] ?? 0;
        const prevY = height - 8 - prevFrac * (height - 20);
        const midX = (prevX + x) / 2;
        const midY = (prevY + y) / 2;
        this.ctx.quadraticCurveTo(prevX, prevY, midX, midY);
      }
    }
    this.ctx.lineTo(width, height);
    this.ctx.closePath();

    const fillGrad = this.ctx.createLinearGradient(0, 0, 0, height);
    if (this.isLive) {
      fillGrad.addColorStop(0, "rgba(187, 154, 247, 0.25)");
      fillGrad.addColorStop(1, "rgba(122, 162, 247, 0.0)");
    } else {
      fillGrad.addColorStop(0, "rgba(169, 177, 214, 0.08)");
      fillGrad.addColorStop(1, "rgba(169, 177, 214, 0.0)");
    }
    this.ctx.fillStyle = fillGrad;
    this.ctx.fill();

    // Line stroke
    this.ctx.beginPath();
    for (let i = 0; i < count; i++) {
      const frac = fractions[i] ?? 0;
      const x = i * step;
      const y = height - 8 - frac * (height - 20);
      if (i === 0) {
        this.ctx.moveTo(x, y);
      } else {
        const prevX = (i - 1) * step;
        const prevFrac = fractions[i - 1] ?? 0;
        const prevY = height - 8 - prevFrac * (height - 20);
        const midX = (prevX + x) / 2;
        const midY = (prevY + y) / 2;
        this.ctx.quadraticCurveTo(prevX, prevY, midX, midY);
      }
    }

    const strokeGrad = this.ctx.createLinearGradient(0, 0, width, 0);
    if (this.isLive) {
      strokeGrad.addColorStop(0, "#7aa2f7");
      strokeGrad.addColorStop(0.5, "#bb9af7");
      strokeGrad.addColorStop(1, "#7dcfff");
    } else {
      strokeGrad.addColorStop(0, "rgba(169, 177, 214, 0.35)");
      strokeGrad.addColorStop(1, "rgba(169, 177, 214, 0.15)");
    }
    this.ctx.strokeStyle = strokeGrad;
    this.ctx.lineWidth = 2.5;
    this.ctx.stroke();
  }
}
