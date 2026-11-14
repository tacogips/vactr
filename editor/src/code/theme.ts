import { FALLBACK_PALETTE as EM_FALLBACK, parseColor, readPalette as readEmakiPalette, type Palette, type Rgba } from 'emaki/engine';
export { rgbaCss, type Rgba } from 'emaki/engine';
export type VactrPalette = Palette & { diagnostic: Rgba; callHead: Rgba };
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
} as Record<string, Rgba>;
const LEGACY_TOKENS = {
  'vact-tok-comment': '#7a7f87', 'vact-tok-directive': '#b07bd8', 'vact-tok-keyword': '#d08770',
  'vact-tok-number': '#88c0d0', 'vact-tok-string': '#a3be8c', 'vact-tok-path': '#a3be8c',
  'vact-tok-head': '#ebcb8b', 'vact-tok-bracket': '#8a8f98',
} as const;


export const GPU_TOKEN_COLORS: Readonly<Record<string, string>> = LEGACY_TOKENS;
export const FALLBACK_PALETTE: VactrPalette = {
  ...EM_FALLBACK, ...LEGACY_FIELDS, diagnostic: LEGACY_FIELDS.diagnostic!, callHead: LEGACY_FIELDS.callHead!,
  token: Object.fromEntries(Object.entries(LEGACY_TOKENS).map(([key, value]) => [key, parseColor(value)!])),
};
export function readPalette(root: Element | null): VactrPalette {
  const style = root?.ownerDocument.defaultView?.getComputedStyle(root);
  if (!style) return FALLBACK_PALETTE;
  const base = readEmakiPalette(root, { token: TOKEN_PROPERTIES, fields: FIELD_PROPERTIES });
  const diagnostic = parseColor(style.getPropertyValue(FIELD_PROPERTIES.diagnostic)) ?? FALLBACK_PALETTE.diagnostic;
  const head = parseColor(style.getPropertyValue(FIELD_PROPERTIES.callHead)) ?? FALLBACK_PALETTE.callHead;
  return { ...base, token: { ...FALLBACK_PALETTE.token, ...base.token }, diagnostic, callHead: [head[0], head[1], head[2], 0.6] };
}
