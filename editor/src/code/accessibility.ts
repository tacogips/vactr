import type { CodeSurface } from '../app/apis';

export const INPUT_WINDOW_LIMIT = 8192;
const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
export function boundary(text: string, pos: number, bias: -1 | 1 = -1): number {
  pos = Math.max(0, Math.min(text.length, pos));
  // Segment only the containing line; CRLF is one navigation unit.
  if (text[pos - 1] === '\r' && text[pos] === '\n') return pos + (bias < 0 ? -1 : 1);
  let start = pos;
  while (start > 0 && text[start - 1] !== '\n' && text[start - 1] !== '\r') start--;
  let end = pos;
  while (end < text.length && text[end] !== '\n' && text[end] !== '\r') end++;
  for (const s of graphemes.segment(text.slice(start, end))) {
    const a = start + s.index, b = a + s.segment.length;
    if (a < pos && pos < b) return bias < 0 ? a : b;
    if (a >= pos) break;
  }
  return pos;
}
export interface InputWindow { start: number; end: number; value: string; anchor: number; head: number; outside: boolean }
export function surroundingWindow(surface: CodeSurface): InputWindow {
  const text = surface.state.doc.toString(), selection = surface.state.selection.main;
  let start = boundary(text, Math.max(0, selection.head - INPUT_WINDOW_LIMIT / 2), 1);
  let end = boundary(text, Math.min(text.length, start + INPUT_WINDOW_LIMIT), -1);
  if (end === text.length) start = boundary(text, Math.max(0, end - INPUT_WINDOW_LIMIT), 1);
  // A single oversized grapheme cannot fit: retain an empty caret window.
  if (end < start || selection.head < start || selection.head > end) start = end = boundary(text, selection.head, -1);
  return { start, end, value: text.slice(start, end), anchor: selection.anchor, head: selection.head,
    outside: selection.from < start || selection.to > end };
}

/** Accessible DOM bridge only; the canvas owns all visible source and feedback. */
export class AccessibilityBridge {
  readonly textarea: HTMLTextAreaElement;
  readonly status: HTMLElement;
  window: InputWindow;
  private projected = { start: 0, end: 0, direction: 'none' as string };
  constructor(private surface: CodeSurface, container: HTMLElement, label = 'Code editor') {
    this.textarea = document.createElement('textarea');
    this.textarea.setAttribute('aria-label', label);
    this.textarea.setAttribute('aria-multiline', 'true');
    this.textarea.setAttribute('autocapitalize', 'off'); this.textarea.spellcheck = false;
    this.textarea.wrap = 'off'; this.textarea.autocomplete = 'off';
    Object.assign(this.textarea.style, { position: 'fixed', opacity: '0', color: 'transparent',
      background: 'transparent', caretColor: 'transparent', width: '1px', height: '20px',
      padding: '0', border: '0', resize: 'none', overflow: 'hidden' });
    this.status = document.createElement('div'); this.status.setAttribute('role', 'status');
    this.status.setAttribute('aria-live', 'polite');
    Object.assign(this.status.style, { position: 'absolute', width: '1px', height: '1px', overflow: 'hidden', clipPath: 'inset(50%)' });
    container.append(this.textarea, this.status);
    this.window = surroundingWindow(surface); this.refresh();
  }
  refresh(): void {
    this.window = surroundingWindow(this.surface);
    this.textarea.value = this.window.value;
    const { start, end, anchor, head } = this.window;
    const a = Math.max(start, Math.min(end, anchor)) - start, h = Math.max(start, Math.min(end, head)) - start;
    this.textarea.setSelectionRange(Math.min(a, h), Math.max(a, h), anchor > head ? 'backward' : 'forward');
    this.projected = { start: this.textarea.selectionStart, end: this.textarea.selectionEnd, direction: this.textarea.selectionDirection };
    this.position();
  }
  position(): void {
    const rect = this.surface.coordsAtPos(this.surface.state.selection.main.head);
    if (!rect) return;
    const view = window.visualViewport;
    // Geometry is viewport CSS coordinates. Clamp into the visible keyboard viewport.
    const left = view?.offsetLeft ?? 0, top = view?.offsetTop ?? 0;
    this.textarea.style.left = `${Math.max(left, Math.min(left + (view?.width ?? window.innerWidth) - 1, rect.left))}px`;
    this.textarea.style.top = `${Math.max(top, Math.min(top + (view?.height ?? window.innerHeight) - (rect.bottom - rect.top), rect.top))}px`;
    this.textarea.style.height = `${Math.max(1, rect.bottom - rect.top)}px`;
  }
  readSelection(): { anchor: number; head: number } | null {
    const el = this.textarea;
    if (el.selectionStart === this.projected.start && el.selectionEnd === this.projected.end && el.selectionDirection === this.projected.direction) return null;
    const backward = el.selectionDirection === 'backward';
    const text = this.surface.state.doc.toString();
    return { anchor: boundary(text, this.window.start + (backward ? el.selectionEnd : el.selectionStart), backward ? 1 : -1),
      head: boundary(text, this.window.start + (backward ? el.selectionStart : el.selectionEnd), backward ? -1 : 1) };
  }
  announce(message: string, visible = false): void {
    this.status.textContent = message.slice(0, 256);
    Object.assign(this.status.style, visible
      ? { position: 'absolute', width: 'auto', height: 'auto', clipPath: 'none' }
      : { position: 'absolute', width: '1px', height: '1px', clipPath: 'inset(50%)' });
  }
  focus(): void { this.textarea.focus({ preventScroll: true }); }
  dispose(): void { this.textarea.remove(); this.status.remove(); }
}
