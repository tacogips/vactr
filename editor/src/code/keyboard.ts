import { isolateHistory } from '@codemirror/commands';
import type { Text } from '@codemirror/state';
import { CodeSurface } from './surface';
import { boundary } from './accessibility';
import type { PhaseTimer } from './frame';
import { isApplePlatform, stopShortcut } from '../ui/stop-keys';

const words = new Intl.Segmenter(undefined, { granularity: 'word' });
export function wordRange(text: string, position: number): { from: number; to: number } {
  for (const s of words.segment(text)) if (s.index <= position && position < s.index + s.segment.length) return { from: s.index, to: s.index + s.segment.length };
  return { from: position, to: position };
}
export function wordRangeAt(doc: Text, position: number): { from: number; to: number } {
  const line = doc.lineAt(Math.max(0, Math.min(doc.length, Math.floor(position))));
  const local = Math.max(0, Math.min(line.length, position - line.from));
  const from = Math.max(0, local - 32_768), to = Math.min(line.length, local + 32_768);
  const range = wordRange(line.text.slice(from, to), local - from);
  return { from: line.from + from + range.from, to: line.from + from + range.to };
}
export function wordMove(text: string, position: number, direction: -1 | 1): number {
  if (direction < 0) {
    let previous = 0;
    for (const s of words.segment(text)) {
      if (s.index >= position) break;
      if (s.isWordLike) previous = s.index;
    }
    return previous;
  }
  for (const s of words.segment(text)) if (s.isWordLike && s.index + s.segment.length > position) return s.index + s.segment.length;
  return text.length;
}
export function lineBoundary(doc: Text, position: number, bias: -1 | 1): number {
  const line = doc.lineAt(Math.max(0, Math.min(doc.length, position)));
  const first = Math.max(1, Math.min(doc.lines, line.number));
  const last = position > line.to ? Math.min(doc.lines, first + 1) : first;
  const from = doc.line(first).from, to = doc.line(last).to;
  const local = doc.sliceString(from, to);
  return from + boundary(local, position - from, bias);
}
export function boundaryAt(doc: Text, position: number, bias: -1 | 1 = -1): number {
  const line = doc.lineAt(Math.max(0, Math.min(doc.length, Math.floor(position))));
  return line.from + boundary(line.text, Math.max(0, Math.min(line.length, position - line.from)), bias);
}
function wordMoveAt(doc: Text, position: number, direction: -1 | 1): number {
  const line = doc.lineAt(Math.max(0, Math.min(doc.length, position))), local = position - line.from;
  let remaining = 65_536;
  if (direction < 0) {
    for (let number = line.number; number >= 1 && remaining > 0; number -= 1) {
      const current = doc.line(number);
      const end = number === line.number ? Math.max(0, Math.min(current.length, local)) : current.length;
      const start = Math.max(0, end - remaining);
      const text = current.text.slice(start, end);
      let previous = -1;
      for (const part of words.segment(text)) if (part.isWordLike && part.index < text.length) previous = part.index;
      if (previous >= 0) return current.from + start + previous;
      remaining -= text.length + (number > 1 ? 1 : 0);
      if (start > 0 || remaining <= 0) return current.from + start;
    }
    return 0;
  }
  for (let number = line.number; number <= doc.lines && remaining > 0; number += 1) {
    const current = doc.line(number);
    const start = number === line.number ? Math.max(0, Math.min(current.length, local)) : 0;
    const end = Math.min(current.length, start + remaining);
    const text = current.text.slice(start, end);
    for (const part of words.segment(text)) if (part.isWordLike) return current.from + start + part.index + part.segment.length;
    remaining -= text.length + (number < doc.lines ? 1 : 0);
    if (end < current.length || remaining <= 0) return current.from + current.length;
  }
  return doc.length;
}
export interface KeyboardOptions {
  composing?: () => boolean;
  evalSelection?: () => void;
  evalAll?: () => void;
  stopAll?: () => void;
  cut?: () => void;
  scrollCaret?: () => void;
  phases?: PhaseTimer | null;
}
export class KeyboardController {
  constructor(private surface: CodeSurface, private options: KeyboardOptions = {}) {}
  setPhases(phases: PhaseTimer | null): void { this.options.phases = phases; }
  handle(event: KeyboardEvent): boolean {
    if (event.isComposing || event.keyCode === 229 || this.options.composing?.() || this.surface.compositionRange) return false;
    this.options.phases?.begin('input');
    try {
    const mod = event.metaKey || event.ctrlKey, key = event.key.toLowerCase();
    const stopAction = stopShortcut(event, isApplePlatform());
    let handled = true;
    if (mod && key === 'enter') { if (event.shiftKey) this.options.evalAll?.(); else this.options.evalSelection?.(); }
    else if (stopAction === 'stop-all') this.options.stopAll?.();
    else if (stopAction === 'cut') this.options.cut?.();
    else if (mod && key === 'a') this.surface.dispatch({ selection: { anchor: 0, head: this.surface.state.doc.length } });
    else if (mod && key === 'z') { if (event.shiftKey) this.surface.redo(); else this.surface.undo(); }
    else if (event.ctrlKey && key === 'y') this.surface.redo();
    else if (key === 'backspace' || key === 'delete') this.delete(key === 'backspace' ? -1 : 1, event.altKey || event.ctrlKey);
    else if (key === 'tab' && !mod && !event.altKey) {
      const s = this.surface.state.selection.main;
      this.surface.dispatch({ changes: { from: s.from, to: s.to, insert: '\t' }, selection: { anchor: s.from + 1 }, userEvent: 'input.type' });
    } else if (['arrowleft', 'arrowright', 'arrowup', 'arrowdown', 'home', 'end'].includes(key)) {
      const s = this.surface.state.selection.main, doc = this.surface.state.doc;
      const left = key === 'arrowleft', right = key === 'arrowright', backward = left || key === 'arrowup' || key === 'home';
      let position = s.head;
      if (!event.shiftKey && !s.empty && (left || right) && !mod && !event.altKey) position = left ? s.from : s.to;
      else if (key === 'home' || key === 'end') {
        const line = this.surface.state.doc.lineAt(position);
        position = mod ? (key === 'home' ? 0 : doc.length) : (key === 'home' ? line.from : line.to);
      } else if (left || right) {
        position = event.metaKey ? (left ? this.surface.state.doc.lineAt(position).from : this.surface.state.doc.lineAt(position).to)
          : event.ctrlKey || event.altKey ? wordMoveAt(doc, position, left ? -1 : 1)
          : lineBoundary(doc, position + (left ? -1 : 1), left ? -1 : 1);
      } else if (mod) position = backward ? 0 : doc.length;
      else {
        const rect = this.surface.coordsAtPos(position);
        const mapped = rect && this.surface.posAtCoords({ x: rect.left, y: (rect.top + rect.bottom) / 2 + (backward ? -1 : 1) * (rect.bottom - rect.top) });
        if (mapped != null) position = mapped;
        else {
          const line = this.surface.state.doc.lineAt(position), n = Math.max(1, Math.min(this.surface.state.doc.lines, line.number + (backward ? -1 : 1)));
          const target = this.surface.state.doc.line(n);
          position = lineBoundary(doc, Math.min(target.to, target.from + position - line.from), -1);
        }
      }
      this.surface.dispatch({ selection: { anchor: event.shiftKey ? s.anchor : position, head: position } });
    } else handled = false;
    if (handled) { event.preventDefault(); this.options.scrollCaret?.(); }
    return handled;
    } finally { this.options.phases?.end('input'); }
  }
  delete(direction: -1 | 1, word = false): void {
    if (this.options.composing?.() || this.surface.compositionRange) return;
    const s = this.surface.state.selection.main, doc = this.surface.state.doc;
    const end = word ? wordMoveAt(doc, s.head, direction) : lineBoundary(doc, s.head + direction, direction);
    const from = s.empty ? Math.min(s.head, end) : s.from, to = s.empty ? Math.max(s.head, end) : s.to;
    if (from === to) return;
    this.surface.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: direction < 0 ? 'delete.backward' : 'delete.forward',
      annotations: word ? isolateHistory.of('full') : [] });
    this.options.scrollCaret?.();
  }
}
