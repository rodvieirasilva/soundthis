export class CanvasVisualizer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private analyser: AnalyserNode | null = null;
  private dataArray: Uint8Array<ArrayBuffer> | null = null;
  private animationId: number | null = null;
  private isLive = false;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Unable to obtain 2D canvas context");
    }
    this.ctx = context;
    this.handleResize();
    window.addEventListener("resize", () => this.handleResize());
  }

  public setAnalyser(analyser: AnalyserNode | null): void {
    this.analyser = analyser;
    if (analyser) {
      const bufferLength = analyser.frequencyBinCount;
      this.dataArray = new Uint8Array(bufferLength);
    } else {
      this.dataArray = null;
    }
  }

  public setLive(live: boolean): void {
    this.isLive = live;
  }

  public start(): void {
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

  private handleResize(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.ctx.resetTransform?.();
    this.ctx.scale(dpr, dpr);
  }

  private draw(): void {
    const width = this.canvas.getBoundingClientRect().width;
    const height = this.canvas.getBoundingClientRect().height;

    this.ctx.clearRect(0, 0, width, height);

    const barCount = 32;
    const barSpacing = 4;
    const totalSpacing = (barCount - 1) * barSpacing;
    const barWidth = Math.max(2, (width - totalSpacing) / barCount);

    if (this.analyser && this.dataArray && this.isLive) {
      this.analyser.getByteFrequencyData(this.dataArray);
    }

    for (let i = 0; i < barCount; i++) {
      let rawVal = 0;
      if (this.dataArray && this.isLive) {
        // Map frequency ranges logarithmically
        const binIndex = Math.min(
          this.dataArray.length - 1,
          Math.floor(Math.pow(i / barCount, 1.4) * this.dataArray.length)
        );
        rawVal = this.dataArray[binIndex] || 0;
      }

      const fraction = rawVal / 255.0;
      const minH = 4;
      const maxH = height - 8;
      const barH = minH + fraction * (maxH - minH);

      const x = i * (barWidth + barSpacing);
      const y = (height - barH) / 2;

      const grad = this.ctx.createLinearGradient(0, y, 0, y + barH);
      if (this.isLive && fraction > 0.05) {
        grad.addColorStop(0, "#7aa2f7");
        grad.addColorStop(1, "#bb9af7");
      } else {
        grad.addColorStop(0, "rgba(169, 177, 214, 0.15)");
        grad.addColorStop(1, "rgba(169, 177, 214, 0.05)");
      }

      this.ctx.fillStyle = grad;
      this.ctx.beginPath();
      this.ctx.roundRect(x, y, barWidth, barH, 2);
      this.ctx.fill();
    }
  }
}
