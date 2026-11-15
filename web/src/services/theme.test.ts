import { describe, it, expect, beforeEach, vi } from 'vitest';
import { applyTheme, fetchAndApplyTheme, getContrastColor, hexToRgba, ThemePalette } from './theme';

describe('Theme Service', () => {
  beforeEach(() => {
    document.documentElement.style.cssText = '';
    vi.restoreAllMocks();
  });

  it('converts 6-char hex to rgba correctly', () => {
    expect(hexToRgba('#7aa2f7', 0.25)).toBe('rgba(122, 162, 247, 0.25)');
    expect(hexToRgba('7aa2f7', 0.5)).toBe('rgba(122, 162, 247, 0.5)');
    expect(hexToRgba('invalid', 0.5)).toBe('invalid');
  });

  it('determines contrast color based on luminance', () => {
    expect(getContrastColor('#ffffff')).toBe('#1a1b26');
    expect(getContrastColor('#eff1f5')).toBe('#1a1b26');
    expect(getContrastColor('#000000')).toBe('#ffffff');
    expect(getContrastColor('#1a1b26')).toBe('#ffffff');
    expect(getContrastColor('invalid')).toBe('#ffffff');
  });

  it('applies palette to document.documentElement CSS variables', () => {
    const palette: ThemePalette = {
      background: '#123456',
      lighter_background: '#234567',
      selection: '#345678',
      bright_foreground: '#ffffff',
      foreground: '#cccccc',
      accent: '#ff007f',
      green: '#00ff00',
      yellow: '#ffff00',
      red: '#ff0000',
    };

    applyTheme(palette);

    const root = document.documentElement;
    expect(root.style.getPropertyValue('--bg')).toBe('#123456');
    expect(root.style.getPropertyValue('--bg-card')).toBe('#234567');
    expect(root.style.getPropertyValue('--bg-hover')).toBe('#345678');
    expect(root.style.getPropertyValue('--fg')).toBe('#ffffff');
    expect(root.style.getPropertyValue('--fg-dim')).toBe('#cccccc');
    expect(root.style.getPropertyValue('--accent')).toBe('#ff007f');
    expect(root.style.getPropertyValue('--accent-fg')).toBe('#ffffff');
    expect(root.style.getPropertyValue('--border')).toBe('rgba(255, 0, 127, 0.25)');
    expect(root.style.getPropertyValue('--green')).toBe('#00ff00');
    expect(root.style.getPropertyValue('--yellow')).toBe('#ffff00');
    expect(root.style.getPropertyValue('--red')).toBe('#ff0000');
  });

  it('fetches and applies theme from API endpoint', async () => {
    const mockPalette: ThemePalette = {
      accent: '#00ffff',
      background: '#000000',
    };

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: true,
      json: async () => mockPalette,
    } as Response);

    const result = await fetchAndApplyTheme('/api/theme');
    expect(result).toEqual(mockPalette);
    expect(document.documentElement.style.getPropertyValue('--accent')).toBe('#00ffff');
    expect(document.documentElement.style.getPropertyValue('--bg')).toBe('#000000');
  });

  it('handles fetch error gracefully without throwing', async () => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new Error('Network error'));

    const result = await fetchAndApplyTheme('/api/theme');
    expect(result).toBeNull();
  });

  it('handles non-ok response gracefully', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: false,
    } as Response);

    const result = await fetchAndApplyTheme('/api/theme');
    expect(result).toBeNull();
  });
});
