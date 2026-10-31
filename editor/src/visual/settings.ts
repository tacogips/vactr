import type { Text } from '@codemirror/state';
import type { CodeSurface, CodeSurfaceUpdate } from '../app/apis';

export type VisualizerKind = 'oscilloscope' | 'spectrum' | 'spectrogram' | 'level';
export interface BackgroundSettings {
  kind: 'none' | 'image' | 'video' | 'shader';
  src?: string;
  output: 0 | 1 | 2 | 3;
  opacity: number;
  fit: 'cover' | 'contain';
  blur: number;
  scrim: number;
}
export interface VisualizerSettings {
  kind: VisualizerKind;
  opacity: number;
  position: 'bottom' | 'top' | 'full';
}
export interface EditorSettings {
  background: BackgroundSettings | null;
  visualizers: VisualizerSettings[];
}
export interface ParsedEditorSettings {
  settings: EditorSettings;
  errors: string[];
}

const PREFIX = /^\s*#@\s+editor(?:\s|$)/;
const MAX_LINE = 65_536;
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
function object(v: unknown, keys: string[], name: string): Record<string, unknown> {
  if (!record(v)) throw new Error(`${name} must be an object`);
  for (const key of Object.keys(v)) if (!keys.includes(key)) throw new Error(`Unknown ${name} option: ${key}`);
  return v;
}
function choice<T extends string>(v: unknown, values: readonly T[], fallback: T, name: string): T {
  if (v === undefined) return fallback;
  if (typeof v !== 'string' || !values.includes(v as T)) throw new Error(`${name} must be ${values.join(', ')}`);
  return v as T;
}
function range(v: unknown, fallback: number, max: number, name: string): number {
  if (v === undefined) return fallback;
  if (typeof v !== 'number' || !Number.isFinite(v) || v < 0 || v > max) throw new Error(`${name} must be between 0 and ${max}`);
  return v;
}

/** Reject executable/non-media schemes before assigning any DOM source. */
export function mediaUrl(src: string, kind: 'image' | 'video', base: string): string {
  const url = new URL(src, base);
  if (url.protocol === 'http:' || url.protocol === 'https:' || url.protocol === 'blob:') return url.href;
  if (url.protocol === 'data:' && new RegExp(`^data:${kind}/`, 'i').test(src)) return url.href;
  // Tauri's packaged page can use asset: or tauri: for relative resources only.
  if (!/^[a-z][a-z0-9+.-]*:/i.test(src) && ['asset:', 'tauri:'].includes(url.protocol)) return url.href;
  throw new Error(`Unsupported ${kind} URL scheme: ${url.protocol}`);
}

function background(value: unknown): BackgroundSettings {
  const v = object(value, ['kind', 'src', 'output', 'opacity', 'fit', 'blur', 'scrim'], 'background');
  const kind = choice(v.kind, ['none', 'image', 'video', 'shader'] as const, 'none', 'background.kind');
  if (!Object.hasOwn(v, 'kind')) throw new Error('background.kind is required');
  let src: string | undefined;
  if (kind === 'image' || kind === 'video') {
    if (typeof v.src !== 'string' || !v.src.trim() || v.src.length > MAX_LINE) throw new Error('background.src must be a nonempty media URL');
    mediaUrl(v.src, kind, 'https://editor.invalid/');
    src = v.src;
  } else if (v.src !== undefined) throw new Error('background.src is only valid for image or video');
  const out = v.output ?? 0;
  if (!Number.isInteger(out) || (out as number) < 0 || (out as number) > 3) throw new Error('background.output must be 0, 1, 2, or 3');
  return { kind, ...(src ? { src } : {}), output: out as 0 | 1 | 2 | 3,
    opacity: range(v.opacity, 1, 1, 'background.opacity'),
    fit: choice(v.fit, ['cover', 'contain'] as const, 'cover', 'background.fit'),
    blur: range(v.blur, 0, 32, 'background.blur'), scrim: range(v.scrim, 0.55, 1, 'background.scrim') };
}
function visualizers(value: unknown): VisualizerSettings[] {
  if (!Array.isArray(value) || value.length > 4) throw new Error('visualizers must be an array of at most four overlays');
  return value.map((value) => {
    const v = object(value, ['kind', 'opacity', 'position'], 'visualizer');
    if (!Object.hasOwn(v, 'kind')) throw new Error('visualizer.kind is required');
    return { kind: choice(v.kind, ['oscilloscope', 'spectrum', 'spectrogram', 'level'] as const, 'oscilloscope', 'visualizer.kind'),
      opacity: range(v.opacity, 0.6, 1, 'visualizer.opacity'),
      position: choice(v.position, ['bottom', 'top', 'full'] as const, 'bottom', 'visualizer.position') };
  });
}

interface SettingLine { from: number; to: number; number: number; text: string }
function parseLines(lines: readonly SettingLine[]): ParsedEditorSettings {
  const settings: EditorSettings = { background: null, visualizers: [] };
  const errors: string[] = [];
  for (const line of lines) {
    try {
      if (line.text.length > MAX_LINE) throw new Error('editor comment is too long');
      const body = line.text.replace(PREFIX, '').trim();
      const value = object(JSON.parse(body), ['background', 'visualizers'], 'editor');
      // Validate the whole line before merging either key.
      const bg = Object.hasOwn(value, 'background') ? background(value.background) : undefined;
      const overlays = Object.hasOwn(value, 'visualizers') ? visualizers(value.visualizers) : undefined;
      if (bg !== undefined) settings.background = bg;
      if (overlays !== undefined) settings.visualizers = overlays;
    } catch (error) { errors.push(`Line ${line.number}: ${error instanceof Error ? error.message : String(error)}`); }
  }
  return { settings, errors };
}

/** Only own-line comments count. Vactr strings are confined to one physical line. */
export function parseEditorSettings(source: string): ParsedEditorSettings {
  let from = 0;
  const lines = source.split('\n').flatMap((text, index) => {
    const entry = { from, to: from + text.length, number: index + 1, text };
    from += text.length + 1;
    return PREFIX.test(text) ? [entry] : [];
  });
  return parseLines(lines);
}
function settingLines(doc: Text): SettingLine[] {
  const lines: SettingLine[] = [];
  let from = 0, number = 1;
  for (const text of doc.iterLines()) {
    if (PREFIX.test(text)) lines.push({ from, to: from + text.length, number, text });
    from += text.length + 1; number++;
  }
  return lines;
}

/** Parse each directive independently so inline widgets retain their source line. */
export function editorSettingLines(doc: Text): { line: number; settings: EditorSettings }[] {
  return settingLines(doc).flatMap((entry) => {
    const parsed = parseLines([entry]);
    return parsed.errors.length ? [] : [{ line: entry.number, settings: parsed.settings }];
  });
}

/** Ordinary edits inspect changed lines; full scans happen only when directives can change. */
export class EditorSettingsSource {
  private doc: Text;
  private lines: SettingLine[];
  private signature = '';
  private readonly off: () => void;
  constructor(surface: CodeSurface, private readonly onChange: (parsed: ParsedEditorSettings) => void) {
    this.doc = surface.state.doc;
    this.lines = settingLines(this.doc);
    this.publish();
    this.off = surface.subscribe((update) => this.changed(update));
  }
  private changed(update: CodeSurfaceUpdate): void {
    if (!update.docChanged) return;
    const next = update.state.doc;
    let relevant = false;
    update.changes.iterChangedRanges((fromA, toA, fromB, toB) => {
      if (this.lines.some((line) => fromA <= line.to && toA >= line.from) ||
          PREFIX.test(this.doc.lineAt(fromA).text) || PREFIX.test(this.doc.lineAt(toA).text) ||
          PREFIX.test(next.lineAt(fromB).text) || PREFIX.test(next.lineAt(toB).text)) relevant = true;
      if (!relevant) {
        for (const line of next.iterLines(next.lineAt(fromB).number, next.lineAt(toB).number + 1)) {
          if (PREFIX.test(line)) { relevant = true; break; }
        }
      }
    });
    this.doc = next;
    if (relevant) { this.lines = settingLines(next); this.publish(); }
    else {
      let moved = false;
      this.lines = this.lines.map((line) => {
      const from = update.changes.mapPos(line.from, 1);
      const number = next.lineAt(from).number;
      moved ||= number !== line.number;
      return { ...line, from, to: from + line.text.length, number };
    });
      if (moved) this.publish();
    }
  }
  private publish(): void {
    const parsed = parseLines(this.lines);
    const signature = JSON.stringify(parsed);
    if (signature !== this.signature) { this.signature = signature; this.onChange(parsed); }
  }
  dispose(): void { this.off(); }
}
