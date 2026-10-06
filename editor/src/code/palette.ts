export type Rgba = [number, number, number, number];

export interface Palette {
  token: Record<string, Rgba>;
  text: Rgba;
  gutter: Rgba;
  diagnostic: Rgba;
  composition: Rgba;
  callHead: Rgba;
  cursor: Rgba;
  labelFill: Rgba;
  labelText: Rgba;
  handle: Rgba;
}

const TOKEN_PROPERTIES: Readonly<Record<string, string>> = Object.freeze({
  'vact-tok-comment': '--vt-syn-comment',
  'vact-tok-directive': '--vt-syn-directive',
  'vact-tok-keyword': '--vt-syn-keyword',
  'vact-tok-number': '--vt-syn-number',
  'vact-tok-string': '--vt-syn-string',
  'vact-tok-path': '--vt-syn-string',
  'vact-tok-head': '--vt-syn-head',
  'vact-tok-bracket': '--vt-syn-bracket',
});
const FIELD_PROPERTIES = {
  text: '--vt-text', gutter: '--vt-syn-comment', diagnostic: '--vt-danger',
  composition: '--vt-data-1', callHead: '--vt-text-muted', cursor: '--vt-text',
  labelFill: '--vt-raised', labelText: '--vt-syn-head', handle: '--vt-data-1',
} as const;
const LEGACY_FIELDS = {
  text: [216 / 255, 222 / 255, 233 / 255, 1], gutter: [122 / 255, 127 / 255, 135 / 255, 1],
  diagnostic: [0.75, 0.2, 0.25, 1], composition: [0.53, 0.75, 0.82, 1],
  callHead: [0.55, 0.6, 0.66, 0.6], cursor: [0.9, 0.92, 0.94, 1],
  labelFill: [0.15, 0.19, 0.25, 0.95], labelText: [235 / 255, 203 / 255, 139 / 255, 1],
  handle: [0.53, 0.75, 0.82, 1],
} as const;
const LEGACY_TOKENS = {
  'vact-tok-comment': '#7a7f87', 'vact-tok-directive': '#b07bd8', 'vact-tok-keyword': '#d08770',
  'vact-tok-number': '#88c0d0', 'vact-tok-string': '#a3be8c', 'vact-tok-path': '#a3be8c',
  'vact-tok-head': '#ebcb8b', 'vact-tok-bracket': '#8a8f98',
} as const;

function parseColor(value: string): Rgba | null {
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

function parseTokenMap(values: Readonly<Record<string, string>>): Record<string, Rgba> {
  return Object.fromEntries(Object.entries(values).map(([name, value]) => [name, parseColor(value)!]));
}

export const FALLBACK_PALETTE: Palette = Object.freeze({
  token: parseTokenMap(LEGACY_TOKENS),
  ...LEGACY_FIELDS,
}) as Palette;

/** Reads CSS tokens once at mount or theme change; drawing never queries computed style. */
export function readPalette(root: Element | null): Palette {
  if (!root) return FALLBACK_PALETTE;
  const style = root.ownerDocument.defaultView?.getComputedStyle(root);
  if (!style) return FALLBACK_PALETTE;
  const read = (property: string, fallback: Rgba): Rgba => parseColor(style.getPropertyValue(property)) ?? fallback;
  const token = Object.fromEntries(Object.entries(TOKEN_PROPERTIES).map(([name, property]) => [name,
    read(property, FALLBACK_PALETTE.token[name]!)]));
  const fields = Object.fromEntries(Object.entries(FIELD_PROPERTIES).map(([name, property]) => [name,
    read(property, FALLBACK_PALETTE[name as keyof Omit<Palette, 'token'>] as Rgba)])) as Omit<Palette, 'token'>;
  fields.callHead = [fields.callHead[0], fields.callHead[1], fields.callHead[2], 0.6];
  return { token, ...fields };
}

export function rgbaCss([red, green, blue, alpha]: Rgba): string {
  if (alpha === 1) {
    const channel = (value: number): string => Math.round(value * 255).toString(16).padStart(2, '0');
    return `#${channel(red)}${channel(green)}${channel(blue)}`;
  }
  return `rgba(${Math.round(red * 255)}, ${Math.round(green * 255)}, ${Math.round(blue * 255)}, ${alpha})`;
}
