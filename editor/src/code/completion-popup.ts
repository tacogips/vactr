import {
  COMPLETION_USER_EVENT,
  isTriggerChar,
  type CompletionKey,
  type CompletionResult,
  type CompletionSource,
  type CompletionSurface,
} from './completion-types';

export type { CompletionSurface } from './completion-types';

const VISIBLE_ROWS = 12;
const ROW_HEIGHT_PX = 24;

export class CompletionPopup {
  private readonly surface: CompletionSurface;
  private readonly source: CompletionSource;
  private readonly unsubscribers: Array<() => void>;
  private result: CompletionResult | null = null;
  private panel: HTMLDivElement | null = null;
  private rowElements: HTMLDivElement[] = [];
  private selected = 0;
  private requestSequence = 0;
  private inFlight = false;
  private dirty = false;
  private disposed = false;

  constructor(surface: CompletionSurface, source: CompletionSource) {
    this.surface = surface;
    this.source = source;
    this.unsubscribers = [
      surface.onChange((change) => this.handleChange(change)),
      surface.onKey((key) => this.handleKey(key)),
      surface.onBlur(() => this.close()),
      surface.onCompositionStart(() => this.close()),
    ];
  }

  isOpen(): boolean {
    return this.panel !== null;
  }

  selectedIndex(): number {
    return this.selected;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.close();
    for (const unsubscribe of this.unsubscribers.splice(0)) unsubscribe();
  }

  private handleChange(change: { docChanged: boolean; selectionChanged: boolean; userEvent: string | null; inserted: string }): void {
    if (this.disposed) return;
    if (!change.docChanged) {
      if (change.selectionChanged) this.close();
      return;
    }
    if (change.userEvent === COMPLETION_USER_EVENT) return;

    const selection = this.surface.selection();
    if (this.panel) {
      if (this.isTypingOrDeleting(change.userEvent)) {
        if (selection.head >= (this.result?.from ?? 0)) this.request();
        else this.close();
      }
      return;
    }

    if (
      change.userEvent === 'input.type' &&
      isTriggerChar(change.inserted) &&
      selection.anchor === selection.head &&
      !this.surface.isComposing()
    ) {
      this.request();
    }
  }

  private isTypingOrDeleting(userEvent: string | null): boolean {
    return userEvent?.startsWith('input.type') === true || userEvent?.startsWith('delete') === true;
  }

  private handleKey(key: CompletionKey): boolean {
    if (this.disposed) return false;
    if (key === 'Ctrl-Space') {
      if (!this.source.available) return false;
      this.request();
      return true;
    }
    if (!this.panel || !this.result) return false;

    switch (key) {
      case 'ArrowDown':
        this.move(1);
        return true;
      case 'ArrowUp':
        this.move(-1);
        return true;
      case 'PageDown':
        this.move(VISIBLE_ROWS);
        return true;
      case 'PageUp':
        this.move(-VISIBLE_ROWS);
        return true;
      case 'Enter':
      case 'Tab':
        this.accept(this.selected);
        return true;
      case 'Escape':
        this.close();
        return true;
    }
  }

  private request(): void {
    if (this.disposed) return;
    const sequence = ++this.requestSequence;
    if (this.inFlight) {
      this.dirty = true;
      return;
    }
    if (!this.source.available || this.surface.isComposing()) {
      this.close();
      return;
    }
    void this.runRequest(sequence);
  }

  private async runRequest(sequence: number): Promise<void> {
    const text = this.surface.text();
    const cursor = this.surface.selection().head;
    this.inFlight = true;
    try {
      const result = await this.source.complete(text, cursor);
      if (sequence !== this.requestSequence || this.disposed) return;
      if (
        result === null ||
        result.items.length === 0 ||
        this.surface.text() !== text ||
        this.surface.isComposing()
      ) {
        this.close();
        return;
      }
      this.show(result);
    } catch {
      if (sequence === this.requestSequence) this.close();
    } finally {
      this.inFlight = false;
      if (this.dirty && !this.disposed) {
        this.dirty = false;
        const nextSequence = this.requestSequence;
        if (this.source.available && !this.surface.isComposing()) void this.runRequest(nextSequence);
        else this.close();
      }
    }
  }

  private show(result: CompletionResult): void {
    const rect = this.surface.caretRect(result.from);
    if (!rect) {
      this.close();
      return;
    }
    this.result = result;
    this.selected = 0;
    if (!this.panel) {
      const doc = this.surface.popupHost().ownerDocument;
      this.panel = doc.createElement('div');
      this.panel.className = 'vact-completion';
      this.panel.setAttribute('role', 'listbox');
      this.panel.style.position = 'fixed';
      this.panel.style.maxHeight = `${VISIBLE_ROWS * ROW_HEIGHT_PX}px`;
      this.panel.style.overflow = 'auto';
      this.surface.popupHost().appendChild(this.panel);
    }
    this.panel.style.left = `${rect.left}px`;
    this.panel.style.top = `${rect.bottom}px`;
    this.renderRows();
  }

  private renderRows(): void {
    if (!this.panel || !this.result) return;
    const doc = this.panel.ownerDocument;
    this.panel.replaceChildren();
    this.rowElements = this.result.items.map((item, index) => {
      const row = doc.createElement('div');
      row.className = 'vact-completion-item';
      row.style.height = `${ROW_HEIGHT_PX}px`;
      row.style.overflow = 'hidden';
      row.setAttribute('role', 'option');
      row.setAttribute('aria-selected', index === this.selected ? 'true' : 'false');
      const label = doc.createElement('span');
      label.className = 'vact-completion-label';
      label.textContent = item.label;
      const kind = doc.createElement('span');
      kind.className = 'vact-completion-kind';
      kind.textContent = item.kind;
      const detail = doc.createElement('span');
      detail.className = 'vact-completion-detail';
      detail.textContent = item.detail;
      row.append(label, kind, detail);
      row.addEventListener('mousedown', (event) => {
        event.preventDefault();
        this.accept(index);
      });
      this.panel?.appendChild(row);
      return row;
    });
    this.rowElements[this.selected]?.scrollIntoView?.({ block: 'nearest' });
  }

  private move(delta: number): void {
    const count = this.result?.items.length ?? 0;
    if (count === 0) return;
    if (Math.abs(delta) === 1) this.selected = (this.selected + delta + count) % count;
    else this.selected = Math.max(0, Math.min(count - 1, this.selected + delta));
    for (let i = 0; i < this.rowElements.length; i += 1) {
      this.rowElements[i]?.setAttribute('aria-selected', i === this.selected ? 'true' : 'false');
    }
    this.rowElements[this.selected]?.scrollIntoView?.({ block: 'nearest' });
  }

  private accept(index: number): void {
    const result = this.result;
    const item = result?.items[index];
    if (!result || !item) return;
    const head = this.surface.selection().head;
    this.surface.replace(result.from, head, item.insert);
    this.close();
  }

  private close(): void {
    this.requestSequence += 1;
    this.dirty = false;
    this.panel?.remove();
    this.panel = null;
    this.result = null;
    this.rowElements = [];
    this.selected = 0;
  }
}
