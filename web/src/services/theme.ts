export interface ThemePalette {
  mode?: string;
  accent?: string;
  selection?: string;
  muted?: string;
  background?: string;
  dark_background?: string;
  darker_background?: string;
  lighter_background?: string;
  foreground?: string;
  dark_foreground?: string;
  bright_foreground?: string;
  green?: string;
  red?: string;
  yellow?: string;
  blue?: string;
  cyan?: string;
}

export function hexToRgba(hex: string, alpha: number): string {
  const cleanHex = hex.replace('#', '');
  if (cleanHex.length === 6) {
    const r = parseInt(cleanHex.substring(0, 2), 16);
    const g = parseInt(cleanHex.substring(2, 4), 16);
    const b = parseInt(cleanHex.substring(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
  return hex;
}

export function getContrastColor(hexColor: string): string {
  const cleanHex = hexColor.replace('#', '');
  if (cleanHex.length === 6) {
    const r = parseInt(cleanHex.substring(0, 2), 16) / 255;
    const g = parseInt(cleanHex.substring(2, 4), 16) / 255;
    const b = parseInt(cleanHex.substring(4, 6), 16) / 255;
    const toLinear = (c: number) => (c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
    const lum = 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
    return lum > 0.4 ? '#1a1b26' : '#ffffff';
  }
  return '#ffffff';
}

export function applyTheme(palette: ThemePalette): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;

  if (palette.background) {
    root.style.setProperty('--bg', palette.background);
  }
  if (palette.lighter_background) {
    root.style.setProperty('--bg-card', palette.lighter_background);
  }
  if (palette.selection) {
    root.style.setProperty('--bg-hover', palette.selection);
  }
  if (palette.bright_foreground) {
    root.style.setProperty('--fg', palette.bright_foreground);
  }
  if (palette.foreground) {
    root.style.setProperty('--fg-dim', palette.foreground);
  }
  if (palette.accent) {
    root.style.setProperty('--accent', palette.accent);
    root.style.setProperty('--accent-fg', getContrastColor(palette.accent));
    root.style.setProperty('--border', hexToRgba(palette.accent, 0.25));
  }
  if (palette.green) {
    root.style.setProperty('--green', palette.green);
  }
  if (palette.yellow) {
    root.style.setProperty('--yellow', palette.yellow);
  }
  if (palette.red) {
    root.style.setProperty('--red', palette.red);
  }
}

export async function fetchAndApplyTheme(apiUrl = '/api/v1/theme'): Promise<ThemePalette | null> {
  try {
    const res = await fetch(apiUrl);
    if (!res.ok) return null;
    const palette: ThemePalette = await res.json();
    applyTheme(palette);
    return palette;
  } catch {
    return null;
  }
}
