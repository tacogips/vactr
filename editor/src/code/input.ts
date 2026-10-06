import { isolateHistory } from '@codemirror/commands';
import { ChangeSet, Text, type ChangeSet as ChangeSetType } from '@codemirror/state';
import type { CodeAnnotation } from '../app/apis';
import { CodeSurface } from './surface';
import { AccessibilityBridge, boundary } from './accessibility';
import { KeyboardController, type KeyboardOptions } from './keyboard';
import type { PhaseTimer } from './frame';

export interface InputPresentation { doc: Text; changes?: ChangeSetType; readonly text: string; cursor: number; annotations: readonly CodeAnnotation[] }
export interface InputOptions extends KeyboardOptions {
  label?: string;
  onPresentation?: (presentation: InputPresentation) => void;
  onError?: (message: string) => void;
}

/** Platform input reconciliation; the document is never changed by preedit. */
export class InputController {
  readonly accessibility: AccessibilityBridge;
  readonly keyboard: KeyboardController;
  private composing = false;
  private preedit = '';
  private original = '';
  private trailingComposition = false;
  private cachedPresentation: InputPresentation | null = null;
  private pendingChanges: ChangeSetType | undefined;
  private lastStateDoc: Text;
  private presentationKey: { stateDoc: Text; range: { from: number; to: number } | null; preedit: string; composing: boolean; cursor: number } | null = null;
  private stop: () => void;
  private listeners: (() => void)[] = [];
  private disposed = false;
  constructor(readonly surface: CodeSurface, container: HTMLElement, private options: InputOptions = {}) {
    this.lastStateDoc = surface.state.doc;
    this.accessibility = new AccessibilityBridge(surface, container, options.label);
    this.keyboard = new KeyboardController(surface, { ...options, composing: () => this.composing });
    const el = this.accessibility.textarea;
    this.listen(el, 'compositionstart', () => this.withInput(() => { this.flushBeforeHandler(); this.surface.notifyCompositionStart(); this.beginComposition(); }));
    this.listen(el, 'compositionupdate', (event) => this.withInput(() => this.updateComposition((event as CompositionEvent).data)));
    this.listen(el, 'compositionend', (event) => this.withInput(() => this.endComposition((event as CompositionEvent).data)));
    this.listen(el, 'beforeinput', (event) => this.withInput(() => { this.flushBeforeHandler(); this.beforeInput(event as InputEvent); }));
    this.listen(el, 'input', (event) => this.withInput(() => this.input(event as InputEvent)));
    this.listen(el, 'select', () => this.withInput(() => { this.syncSelection(); this.flushBeforeHandler(); }));
    this.listen(document, 'selectionchange', () => {
      if (document.activeElement !== el) return;
      this.withInput(() => { this.syncSelection(); this.flushBeforeHandler(); });
    });
    this.listen(el, 'keydown', (event) => this.withInput(() => {
      this.flushBeforeHandler();
      const key = event as KeyboardEvent;
      if (this.composing && key.key === 'Escape') { key.preventDefault(); this.cancelComposition(); return; }
      if (!this.composing) this.trailingComposition = false;
      if (!this.composing && !key.isComposing && key.keyCode !== 229 && !this.surface.runKeymaps(key)) this.keyboard.handle(key);
    }));
    this.listen(el, 'copy', (event) => this.withInput(() => { this.flushBeforeHandler(); this.clipboard(event as ClipboardEvent, 'copy'); }));
    this.listen(el, 'cut', (event) => this.withInput(() => { this.flushBeforeHandler(); this.clipboard(event as ClipboardEvent, 'cut'); }));
    this.listen(el, 'paste', (event) => this.withInput(() => { this.flushBeforeHandler(); this.clipboard(event as ClipboardEvent, 'paste'); }));
    this.listen(el, 'focus', () => this.withInput(() => this.flushBeforeHandler()));
    this.listen(el, 'blur', () => { this.surface.notifyBlur(); this.cancelComposition(); });
    const position = () => this.accessibility.markPositionDirty();
    this.listen(window, 'resize', position);
    this.listen(window, 'orientationchange', position);
    this.listen(window, 'scroll', position);
    if (window.visualViewport) { this.listen(window.visualViewport, 'resize', position); this.listen(window.visualViewport, 'scroll', position); }
    this.stop = surface.subscribe((update) => {
      const previous = this.cachedPresentation;
      this.pendingChanges = update.docChanged && !this.composing && previous?.doc === this.lastStateDoc ? update.changes : undefined;
      this.lastStateDoc = update.state.doc;
      if (!this.composing) this.accessibility.markDirty();
      this.publish();
    });
    this.publish();
  }
  setPhases(phases: PhaseTimer | null): void { this.options.phases = phases; this.keyboard.setPhases(phases); }
  get isComposing(): boolean { return this.composing; }
  flushBridge(): void {
    const position = this.accessibility.isDirty || this.accessibility.isPositionDirty;
    if (position) this.accessibility.flush({ position: true });
  }
  get presentation(): InputPresentation {
    const stateDoc = this.surface.state.doc, range = this.surface.compositionRange;
    const cursor = this.composing && range ? range.from + this.preedit.length : this.surface.state.selection.main.head;
    const key = this.presentationKey;
    if (key && key.stateDoc === stateDoc && key.composing === this.composing && key.preedit === this.preedit && key.cursor === cursor &&
      key.range?.from === range?.from && key.range?.to === range?.to) return this.cachedPresentation!;
    const doc = this.composing && range ? stateDoc.replace(range.from, range.to, Text.of(this.preedit.split('\n'))) : stateDoc;
    const annotations: readonly CodeAnnotation[] = this.composing && range
      ? [{ from: range.from, to: range.from + this.preedit.length, kind: 'composition' }] : [];
    const presentation = { doc, changes: this.pendingChanges, cursor, annotations } as InputPresentation;
    Object.defineProperty(presentation, 'text', { enumerable: true, get: () => doc.toString() });
    this.presentationKey = { stateDoc, range: range ? { ...range } : null, preedit: this.preedit, composing: this.composing, cursor };
    this.cachedPresentation = presentation;
    return presentation;
  }
  private listen(target: EventTarget, name: string, fn: EventListener): void {
    target.addEventListener(name, fn); this.listeners.push(() => target.removeEventListener(name, fn));
  }
  private flushBeforeHandler(): void {
    if (this.accessibility.isDirty) this.accessibility.flush({ position: false });
  }
  private syncSelection(): void {
    const el = this.accessibility.textarea;
    if (this.composing || el.value !== this.accessibility.window.value) return;
    const selection = this.accessibility.readSelection();
    if (selection) this.surface.dispatch({ selection });
  }
  private withInput<T>(run: () => T): T {
    const phases = this.options.phases;
    phases?.begin('input');
    try { return run(); } finally { phases?.end('input'); }
  }
  private publish(changes?: ChangeSetType): void {
    if (this.disposed) return;
    if (changes) this.pendingChanges = changes;
    this.presentationKey = null;
    this.options.onPresentation?.(this.presentation);
  }
  beginComposition(): void {
    if (this.composing || this.disposed) return;
    this.trailingComposition = false;
    const selection = this.surface.state.selection.main;
    this.original = this.surface.state.doc.sliceString(selection.from, selection.to);
    this.surface.setCompositionRange({ from: selection.from, to: selection.to });
    this.preedit = ''; this.composing = true; this.publish();
  }
  updateComposition(text: string): void {
    if (!this.composing) this.beginComposition();
    const previous = this.presentation, range = this.surface.compositionRange;
    const changes = range ? ChangeSet.of({ from: range.from, to: range.from + this.preedit.length, insert: text }, previous.doc.length) : undefined;
    this.preedit = text; this.publish(changes);
  }
  endComposition(text: string): void {
    if (!this.composing) return;
    const range = this.surface.compositionRange;
    // Empty end data denotes cancellation. Revalidate because independent edits may have mapped the range.
    if (text && range && this.surface.state.doc.sliceString(range.from, range.to) === this.original) {
      const insert = this.surface.state.toText(text);
      this.preedit = insert.toString();
      this.surface.dispatch({ changes: { from: range.from, to: range.to, insert }, selection: { anchor: range.from + insert.length },
        annotations: isolateHistory.of('full'), userEvent: 'input.type.compose.start' });
    } else if (text) this.error('Composition cancelled: source changed');
    this.finishComposition();
  }
  cancelComposition(): void { if (this.composing) this.finishComposition(); }
  private finishComposition(): void {
    this.composing = false; this.preedit = ''; this.trailingComposition = true;
    this.surface.setCompositionRange(null); // commit is recorded before revalidating deferred writes
    this.accessibility.refresh(); this.publish();
  }
  private beforeInput(event: InputEvent): void {
    const kind = event.inputType;
    if (this.trailingComposition && (kind === 'insertFromComposition' || kind === 'insertCompositionText')) {
      if (event.cancelable) event.preventDefault();
      this.accessibility.refresh(); return;
    }
    if (this.composing || event.isComposing) {
      if (!this.composing) this.beginComposition();
      if (event.data != null) this.updateComposition(event.data);
      return; // platform owns the textarea during IME
    }
    this.trailingComposition = false;
    if (!event.cancelable) return; // fallback input reconciles the native mutation
    if (kind === 'historyUndo' || kind === 'historyRedo') {
      event.preventDefault(); if (kind === 'historyUndo') this.surface.undo(); else this.surface.redo(); return;
    }
    if (kind === 'deleteContentBackward' || kind === 'deleteContentForward') {
      event.preventDefault(); this.keyboard.delete(kind === 'deleteContentBackward' ? -1 : 1); return;
    }
    const insert = kind === 'insertLineBreak' || kind === 'insertParagraph' ? '\n' : event.data;
    if ((kind === 'insertText' || kind === 'insertReplacementText' || kind === 'insertLineBreak' || kind === 'insertParagraph') && insert != null) {
      event.preventDefault(); this.replaceSelection(insert, 'input.type');
    }
  }
  private input(event: InputEvent): void {
    if (this.composing || event.isComposing) return;
    if (this.trailingComposition && (event.inputType === 'insertFromComposition' || event.inputType === 'insertCompositionText')) {
      this.accessibility.refresh(); return;
    }
    const old = this.accessibility.window, value = this.accessibility.textarea.value;
    if (old.outside && old.anchor !== old.head) {
      // A clipped selection represents more source than the textarea contains.
      // Strip only unselected context: a minimal diff would drop shared replacement characters.
      const projectedFrom = Math.max(0, Math.min(old.value.length, Math.min(old.anchor, old.head) - old.start));
      const projectedTo = Math.max(0, Math.min(old.value.length, Math.max(old.anchor, old.head) - old.start));
      const suffixLength = old.value.length - projectedTo;
      this.replaceSelection(value.slice(projectedFrom, value.length - suffixLength), 'input.type');
      return;
    }
    if (value === old.value) return;
    // Native fallback (noncancelable beforeinput, dictation, accessibility) reconciles one splice.
    let from = 0;
    while (from < old.value.length && from < value.length && old.value[from] === value[from]) from++;
    let a = old.value.length, b = value.length;
    while (a > from && b > from && old.value[a - 1] === value[b - 1]) { a--; b--; }
    const localFrom = boundary(old.value, from, -1), localTo = boundary(old.value, a, 1);
    const extraLeft = from - localFrom, extraRight = localTo - a;
    const insert = this.surface.state.toText(value.slice(Math.max(0, from - extraLeft), Math.min(value.length, b + extraRight)));
    this.surface.dispatch({ changes: { from: old.start + localFrom, to: old.start + localTo, insert },
      selection: { anchor: old.start + localFrom + insert.length }, userEvent: 'input.type' });
  }
  replaceSelection(text: string, userEvent = 'input.paste'): void {
    if (this.composing) return;
    const s = this.surface.state.selection.main, insert = this.surface.state.toText(text);
    this.surface.dispatch({ changes: { from: s.from, to: s.to, insert }, selection: { anchor: s.from + insert.length },
      userEvent, annotations: userEvent === 'input.paste' ? isolateHistory.of('full') : [] });
    this.options.scrollCaret?.();
  }
  private clipboard(event: ClipboardEvent, operation: 'copy' | 'cut' | 'paste'): void {
    if (this.composing) { event.preventDefault(); return; }
    event.preventDefault();
    try {
      if (!event.clipboardData) throw new Error('Clipboard permission unavailable');
      if (operation === 'paste') this.replaceSelection(event.clipboardData.getData('text/plain'));
      else {
        const s = this.surface.state.selection.main;
        event.clipboardData.setData('text/plain', this.surface.state.doc.sliceString(s.from, s.to));
        if (operation === 'cut' && !s.empty) this.replaceSelection('', 'delete.cut');
      }
    } catch { this.error('Clipboard operation denied'); }
  }
  private error(message: string): void { this.accessibility.announce(message, true); this.options.onError?.(message); }
  focus(): void { this.accessibility.focus(); }
  dispose(): void {
    if (this.disposed) return;
    this.cancelComposition(); this.disposed = true;
    this.stop(); for (const remove of this.listeners) remove(); this.listeners = []; this.accessibility.dispose();
  }
}
