export type Rgba = [number, number, number, number];
export interface Palette {
  token: Record<string, Rgba>;
  text: Rgba; gutter: Rgba; cursor: Rgba; composition: Rgba;
  labelFill: Rgba; labelText: Rgba; handle: Rgba; selection: Rgba; background: Rgba;
}
export const FALLBACK_PALETTE: Palette = {
  token: {}, text: [216 / 255, 222 / 255, 233 / 255, 1],
  gutter: [122 / 255, 127 / 255, 135 / 255, 1], cursor: [0.9, 0.92, 0.94, 1],
  composition: [0.53, 0.75, 0.82, 1], labelFill: [0.15, 0.19, 0.25, 0.95],
  labelText: [235 / 255, 203 / 255, 139 / 255, 1], handle: [0.53, 0.75, 0.82, 1],
  selection: [0.218, 0.4, 0.582, 0.55], background: [0.047, 0.059, 0.082, 1],
};

export function parseColor(value: string): Rgba | null {
  const input = value.trim();
  const hex = /^#([\da-f]{3}|[\da-f]{6}|[\da-f]{8})$/i.exec(input)?.[1];
  if (hex) {
    const digits = hex.length === 3 ? [...hex].map((part) => part + part).join('') : hex;
    const channels = digits.match(/../g)?.map((part) => Number.parseInt(part, 16) / 255) ?? [];
    if (channels.length === 3) channels.push(1);
    return channels.length === 4 ? channels as Rgba : null;
  }
  const rgb = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)$/i.exec(input);
  if (!rgb) return null;
  const channels = [Number(rgb[1]) / 255, Number(rgb[2]) / 255, Number(rgb[3]) / 255, rgb[4] === undefined ? 1 : Number(rgb[4])];
  if (!channels.every((channel) => Number.isFinite(channel) && channel >= 0 && channel <= 1)) return null;
  return channels as Rgba;
}

export function readPalette(root: Element | null, properties: { token?: Record<string, string>; fields?: Partial<Record<keyof Omit<Palette, 'token'>, string>> }): Palette {
  const style = root?.ownerDocument.defaultView?.getComputedStyle(root);
  if (!style) return FALLBACK_PALETTE;
  const result: Palette = { ...FALLBACK_PALETTE, token: {} };
  for (const [name, property] of Object.entries(properties.token ?? {})) {
    const color = parseColor(style.getPropertyValue(property));
    if (color) result.token[name] = color;
  }
  for (const [name, property] of Object.entries(properties.fields ?? {})) {
    if (property) {
      const color = parseColor(style.getPropertyValue(property));
      if (color) result[name as keyof Omit<Palette, 'token'>] = color;
    }
  }
  return result;
}

export function rgbaCss([red, green, blue, alpha]: Rgba): string {
  if (alpha === 1) {
    const channel = (value: number): string => Math.round(value * 255).toString(16).padStart(2, '0');
    return `#${channel(red)}${channel(green)}${channel(blue)}`;
  }
  return `rgba(${Math.round(red * 255)}, ${Math.round(green * 255)}, ${Math.round(blue * 255)}, ${alpha})`;
}
