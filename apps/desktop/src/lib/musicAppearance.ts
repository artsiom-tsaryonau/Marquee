export const musicAccents = [
  { name: 'Mint', value: '#63e6be' },
  { name: 'Sky', value: '#7dd3fc' },
  { name: 'Amber', value: '#fbbf24' },
  { name: 'Rose', value: '#fda4af' },
] as const;

export function musicAccent(value: unknown): string {
  return typeof value === 'string' && /^#[\da-f]{6}$/i.test(value) ? value.toLowerCase() : musicAccents[0].value;
}
function channels(hex: string) { return [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16)); }
function luminance(hex: string) {
  const [r, g, b] = channels(hex).map(c => { const v = c / 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; });
  return r * .2126 + g * .7152 + b * .0722;
}
export function contrastRatio(a: string, b: string): number {
  const x = luminance(a), y = luminance(b);
  return (Math.max(x, y) + .05) / (Math.min(x, y) + .05);
}
export function musicPalette(value: unknown, theme: 'dark' | 'light', surface?: string) {
  const fill = musicAccent(value);
  const ink = contrastRatio(fill, '#000000') >= contrastRatio(fill, '#ffffff') ? '#000000' : '#ffffff';
  const background = surface ?? (theme === 'light' ? '#eef0f7' : '#181826');
  const target = theme === 'light' ? 0 : 255;
  let text = fill;
  for (let step = 1; contrastRatio(text, background) < 4.5 && step <= 100; step++) {
    text = '#' + channels(fill).map(c => Math.round(c + (target - c) * step / 100).toString(16).padStart(2, '0')).join('');
  }
  return { fill, ink, text };
}
