const CLIENT_NAME_KEY = "soundthis_client_name";
const VOLUME_KEY = "soundthis_volume";

export class ClientConfigStore {
  public static getClientName(): string {
    const saved = localStorage.getItem(CLIENT_NAME_KEY);
    if (saved && saved.trim()) return saved.trim();

    const ua = navigator.userAgent;
    let name = "Web Browser";
    if (/android/i.test(ua)) name = "Android Phone";
    else if (/iphone|ipad|ipod/i.test(ua)) name = "iPhone";
    else if (/macintosh|mac os x/i.test(ua)) name = "MacBook";
    else if (/linux/i.test(ua)) name = "Linux Client";
    else if (/windows/i.test(ua)) name = "Windows PC";

    this.setClientName(name);
    return name;
  }

  public static setClientName(name: string): void {
    const clean = name.trim().slice(0, 40) || "Device";
    localStorage.setItem(CLIENT_NAME_KEY, clean);
  }

  public static getVolume(): number {
    const saved = localStorage.getItem(VOLUME_KEY);
    if (saved !== null) {
      const parsed = parseFloat(saved);
      if (!isNaN(parsed) && parsed >= 0 && parsed <= 1) return parsed;
    }
    return 1.0;
  }

  public static setVolume(val: number): void {
    localStorage.setItem(VOLUME_KEY, String(val));
  }
}
