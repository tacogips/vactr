import { isolateHistory } from '@codemirror/commands';
import { CodeSurface } from './surface';
import { boundary } from './accessibility';

const words = new Intl.Segmenter(undefined, { granularity: 'word' });
export function wordRange(text: string, position: number): { from: number; to: number } {
  for (const s of words.segment(text)) if (s.index <= position && position < s.index + s.segment.length) return { from: s.index, to: s.index + s.segment.length };
  return { from: position, to: position };
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
export interface KeyboardOptions {
  composing?: () => boolean;
  evalSelection?: () => void;
  evalAll?: () => void;
  hush?: () => void;
  scrollCaret?: () => void;
}
export class KeyboardController {
  constructor(private surface: CodeSurface, private options: KeyboardOptions = {}) {}
  handle(event: KeyboardEvent): boolean {
    if (event.isComposing || event.keyCode === 229 || this.options.composing?.() || this.surface.compositionRange) return false;
    const mod = event.metaKey || event.ctrlKey, key = event.key.toLowerCase();
    let handled = true;
    if (mod && key === 'enter') { if (event.shiftKey) this.options.evalAll?.(); else this.options.evalSelection?.(); }
    else if (mod && key === '.') this.options.hush?.();
    else if (mod && key === 'a') this.surface.dispatch({ selection: { anchor: 0, head: this.surface.state.doc.length } });
    else if (mod && key === 'z') { if (event.shiftKey) this.surface.redo(); else this.surface.undo(); }
    else if (event.ctrlKey && key === 'y') this.surface.redo();
    else if (key === 'backspace' || key === 'delete') this.delete(key === 'backspace' ? -1 : 1, event.altKey || event.ctrlKey);
    else if (key === 'tab' && !mod && !event.altKey) {
      const s = this.surface.state.selection.main;
      this.surface.dispatch({ changes: { from: s.from, to: s.to, insert: '\t' }, selection: { anchor: s.from + 1 }, userEvent: 'input.type' });
    } else if (['arrowleft', 'arrowright', 'arrowup', 'arrowdown', 'home', 'end'].includes(key)) {
      const s = this.surface.state.selection.main, text = this.surface.state.doc.toString();
      const left = key === 'arrowleft', right = key === 'arrowright', backward = left || key === 'arrowup' || key === 'home';
      let position = s.head;
      if (!event.shiftKey && !s.empty && (left || right) && !mod && !event.altKey) position = left ? s.from : s.to;
      else if (key === 'home' || key === 'end') {
        const line = this.surface.state.doc.lineAt(position);
        position = mod ? (key === 'home' ? 0 : text.length) : (key === 'home' ? line.from : line.to);
      } else if (left || right) {
        position = event.metaKey ? (left ? this.surface.state.doc.lineAt(position).from : this.surface.state.doc.lineAt(position).to)
          : event.ctrlKey || event.altKey ? wordMove(text, position, left ? -1 : 1)
          : boundary(text, position + (left ? -1 : 1), left ? -1 : 1);
      } else if (mod) position = backward ? 0 : text.length;
      else {
        const rect = this.surface.coordsAtPos(position);
        const mapped = rect && this.surface.posAtCoords({ x: rect.left, y: (rect.top + rect.bottom) / 2 + (backward ? -1 : 1) * (rect.bottom - rect.top) });
        if (mapped != null) position = mapped;
        else {
          const line = this.surface.state.doc.lineAt(position), n = Math.max(1, Math.min(this.surface.state.doc.lines, line.number + (backward ? -1 : 1)));
          const target = this.surface.state.doc.line(n);
          position = boundary(text, Math.min(target.to, target.from + position - line.from));
        }
      }
      this.surface.dispatch({ selection: { anchor: event.shiftKey ? s.anchor : position, head: position } });
    } else handled = false;
    if (handled) { event.preventDefault(); this.options.scrollCaret?.(); }
    return handled;
  }
  delete(direction: -1 | 1, word = false): void {
    if (this.options.composing?.() || this.surface.compositionRange) return;
    const s = this.surface.state.selection.main, text = this.surface.state.doc.toString();
    const end = word ? wordMove(text, s.head, direction) : boundary(text, s.head + direction, direction);
    const from = s.empty ? Math.min(s.head, end) : s.from, to = s.empty ? Math.max(s.head, end) : s.to;
    if (from === to) return;
    this.surface.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: direction < 0 ? 'delete.backward' : 'delete.forward',
      annotations: word ? isolateHistory.of('full') : [] });
    this.options.scrollCaret?.();
  }
}
