import type { Text } from '@codemirror/state';
import { editorSettingLines } from '../../visual/settings';

type Span = { start: number; end: number };
export function slotAnchors(input: { firstSrc: ReadonlyMap<string, { span: Span; rev: number }>; mapSpan(span: Span, rev: number): { from: number; to: number } | null; forms: readonly { from: number; to: number }[]; doc: Text }): Map<number, string[]> {
  const result = new Map<number, string[]>();
  for (const [slot, src] of input.firstSrc) {
    const mapped = input.mapSpan(src.span, src.rev);
    if (!mapped) continue;
    const form = input.forms.find((item) => item.from <= mapped.from && mapped.from < item.to);
    let pos = form ? form.to : mapped.from;
    if (form && pos > 0 && input.doc.lineAt(pos).from === pos) pos--;
    const line = input.doc.lineAt(pos).number - 1;
    const slots = result.get(line) ?? [];
    slots.push(slot); result.set(line, slots);
  }
  for (const slots of result.values()) slots.sort();
  return result;
}
export function scopeAnchors(doc: Text): number[] {
  return editorSettingLines(doc).filter(({ settings }) => settings.visualizers.some((v) => v.kind === 'oscilloscope')).map(({ line }) => line - 1);
}
