import { afterEach, describe, expect, it } from 'vitest';
import { COMPLETION_USER_EVENT, type CompletionKey, type CompletionResult, type CompletionSource, type CompletionSurface, type SurfaceChange } from '../../src/code/completion-types';
import { CompletionPopup } from '../../src/code/completion-popup';

interface NodeFs {
  readFileSync(path: string, encoding: 'utf8'): string;
}

interface NodeProcess {
  cwd(): string;
}

const nodeFsModule = 'node:fs';
const proc = (globalThis as unknown as { process: NodeProcess }).process;

interface PendingCall {
  text: string;
  cursor: number;
  resolve(result: CompletionResult | null): void;
  reject(error: unknown): void;
}

class FakeSource implements CompletionSource {
  available = true;
  readonly calls: PendingCall[] = [];

  complete(text: string, cursor: number): Promise<CompletionResult | null> {
    return new Promise((resolve, reject) => this.calls.push({ text, cursor, resolve, reject }));
  }

  resolve(index: number, result: CompletionResult | null): void {
    this.calls[index]?.resolve(result);
  }
}

class FakeSurface implements CompletionSurface {
  doc = '';
  anchor = 0;
  head = 0;
  composing = false;
  rect: { left: number; top: number; bottom: number } | null = { left: 12, top: 20, bottom: 40 };
  readonly host = document.createElement('div');
  readonly replaced: Array<[number, number, string]> = [];
  readonly changeListeners = new Set<(change: SurfaceChange) => void>();
  readonly keyListeners = new Set<(key: CompletionKey) => boolean>();
  readonly blurListeners = new Set<() => void>();
  readonly compositionListeners = new Set<() => void>();

  constructor() {
    document.body.appendChild(this.host);
  }

  text(): string {
    return this.doc;
  }

  selection(): { anchor: number; head: number } {
    return { anchor: this.anchor, head: this.head };
  }

  replace(from: number, to: number, insert: string): void {
    this.replaced.push([from, to, insert]);
    this.doc = `${this.doc.slice(0, from)}${insert}${this.doc.slice(to)}`;
    this.anchor = from + insert.length;
    this.head = this.anchor;
    this.emitChange({ docChanged: true, selectionChanged: true, userEvent: COMPLETION_USER_EVENT, inserted: insert });
  }

  caretRect(_pos: number): { left: number; top: number; bottom: number } | null {
    return this.rect;
  }

  isComposing(): boolean {
    return this.composing;
  }

  onChange(listener: (change: SurfaceChange) => void): () => void {
    this.changeListeners.add(listener);
    return () => this.changeListeners.delete(listener);
  }

  onKey(handler: (key: CompletionKey) => boolean): () => void {
    this.keyListeners.add(handler);
    return () => this.keyListeners.delete(handler);
  }

  onBlur(listener: () => void): () => void {
    this.blurListeners.add(listener);
    return () => this.blurListeners.delete(listener);
  }

  onCompositionStart(listener: () => void): () => void {
    this.compositionListeners.add(listener);
    return () => this.compositionListeners.delete(listener);
  }

  popupHost(): HTMLElement {
    return this.host;
  }

  type(inserted: string, userEvent = 'input.type'): void {
    const from = this.head;
    this.doc = `${this.doc.slice(0, from)}${inserted}${this.doc.slice(from)}`;
    this.anchor = from + inserted.length;
    this.head = this.anchor;
    this.emitChange({ docChanged: true, selectionChanged: true, userEvent, inserted });
  }

  select(pos: number): void {
    this.anchor = pos;
    this.head = pos;
    this.emitChange({ docChanged: false, selectionChanged: true, userEvent: null, inserted: '' });
  }

  emitChange(change: SurfaceChange): void {
    for (const listener of this.changeListeners) listener(change);
  }

  emitKey(key: CompletionKey): boolean {
    return [...this.keyListeners].some((listener) => listener(key));
  }

  blur(): void {
    for (const listener of this.blurListeners) listener();
  }

  compose(value: boolean): void {
    this.composing = value;
    if (value) for (const listener of this.compositionListeners) listener();
  }

  listenerCount(): number {
    return this.changeListeners.size + this.keyListeners.size + this.blurListeners.size + this.compositionListeners.size;
  }
}

function items(count = 3): CompletionResult {
  return {
    context: 'head',
    from: 0,
    to: 1,
    incomplete: false,
    items: Array.from({ length: count }, (_, i) => ({ label: `item-${i}`, kind: 'function' as const, detail: `detail-${i}`, insert: `insert-${i}` })),
  };
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

const surfaces: FakeSurface[] = [];
const popups: CompletionPopup[] = [];

function setup(): { surface: FakeSurface; source: FakeSource; popup: CompletionPopup } {
  const surface = new FakeSurface();
  const source = new FakeSource();
  const popup = new CompletionPopup(surface, source);
  surfaces.push(surface);
  popups.push(popup);
  return { surface, source, popup };
}

afterEach(() => {
  for (const popup of popups.splice(0)) popup.dispose();
  for (const surface of surfaces.splice(0)) surface.host.remove();
});

describe('CompletionPopup', () => {
  it('auto-requests only single identifier, colon and dot input and renders the listbox contract', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    expect(source.calls).toHaveLength(1);
    expect(source.calls[0]).toMatchObject({ text: 'a', cursor: 1 });
    source.resolve(0, items());
    await flush();

    const panel = surface.host.querySelector('.vact-completion');
    expect(panel?.getAttribute('role')).toBe('listbox');
    expect((panel as HTMLElement).style.position).toBe('fixed');
    expect((panel as HTMLElement).style.maxHeight).toBe('288px');
    expect((panel as HTMLElement).style.overflow).toBe('auto');
    expect((panel as HTMLElement).style.left).toBe('12px');
    expect((panel as HTMLElement).style.top).toBe('40px');
    const rows = [...surface.host.querySelectorAll('[role="option"]')];
    expect(rows).toHaveLength(3);
    expect((rows[0] as HTMLElement).style.height).toBe('24px');
    expect(rows[0]?.getAttribute('aria-selected')).toBe('true');
    expect(rows[0]?.querySelector('.vact-completion-label')?.textContent).toBe('item-0');
    expect(rows[0]?.querySelector('.vact-completion-kind')?.textContent).toBe('function');
    expect(rows[0]?.querySelector('.vact-completion-detail')?.textContent).toBe('detail-0');
    expect(popup.isOpen()).toBe(true);

    const colonSurface = new FakeSurface();
    const colonSource = new FakeSource();
    const colonPopup = new CompletionPopup(colonSurface, colonSource);
    surfaces.push(colonSurface);
    popups.push(colonPopup);
    colonSurface.type(':');
    expect(colonSource.calls).toHaveLength(1);
    colonPopup.dispose();

    const dotSurface = new FakeSurface();
    const dotSource = new FakeSource();
    const dotPopup = new CompletionPopup(dotSurface, dotSource);
    surfaces.push(dotSurface);
    popups.push(dotPopup);
    dotSurface.type('.');
    expect(dotSource.calls).toHaveLength(1);
    dotPopup.dispose();
  });

  it('does not auto-request spaces, multi-character insertions, paste, or completion edits', () => {
    const { surface, source } = setup();
    surface.type(' ');
    surface.type('ab', 'input.type');
    surface.type('x', 'input.paste');
    surface.emitChange({ docChanged: true, selectionChanged: true, userEvent: COMPLETION_USER_EVENT, inserted: 'x' });
    expect(source.calls).toHaveLength(0);
  });

  it('does not consume keys while closed and Ctrl-Space requests only when available', () => {
    const { surface, source } = setup();
    for (const key of ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Enter', 'Tab', 'Escape'] as const) {
      expect(surface.emitKey(key)).toBe(false);
    }
    source.available = false;
    expect(surface.emitKey('Ctrl-Space')).toBe(false);
    expect(source.calls).toHaveLength(0);
    source.available = true;
    surface.type(' ');
    expect(surface.emitKey('Ctrl-Space')).toBe(true);
    expect(source.calls).toHaveLength(1);
  });

  it('navigates with wrapping arrows and clamped pages, then accepts with Enter or Tab', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    source.resolve(0, items(20));
    await flush();
    expect(surface.emitKey('ArrowDown')).toBe(true);
    expect(surface.emitKey('ArrowDown')).toBe(true);
    expect(popup.selectedIndex()).toBe(2);
    surface.emitKey('ArrowUp');
    expect(popup.selectedIndex()).toBe(1);
    surface.emitKey('PageDown');
    expect(popup.selectedIndex()).toBe(13);
    surface.emitKey('PageDown');
    expect(popup.selectedIndex()).toBe(19);
    surface.emitKey('ArrowDown');
    expect(popup.selectedIndex()).toBe(0);
    surface.emitKey('ArrowUp');
    expect(popup.selectedIndex()).toBe(19);
    surface.emitKey('PageUp');
    expect(popup.selectedIndex()).toBe(7);
    surface.emitKey('ArrowUp');
    expect(popup.selectedIndex()).toBe(6);
    expect(surface.emitKey('Enter')).toBe(true);
    expect(surface.replaced).toEqual([[0, 1, 'insert-6']]);
    expect(popup.isOpen()).toBe(false);
    expect(surface.emitKey('Tab')).toBe(false);

    const second = setup();
    second.surface.type('x');
    second.source.resolve(0, items());
    await flush();
    expect(second.surface.emitKey('Tab')).toBe(true);
    expect(second.surface.replaced).toEqual([[0, 1, 'insert-0']]);
  });

  it('Escape closes and mousedown prevents focus loss and accepts the clicked item', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    source.resolve(0, items());
    await flush();
    expect(surface.emitKey('Escape')).toBe(true);
    expect(popup.isOpen()).toBe(false);
    expect(surface.emitKey('Escape')).toBe(false);

    surface.type('a');
    source.resolve(1, items());
    await flush();
    const row = surface.host.querySelectorAll('[role="option"]')[1];
    const event = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    row?.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(surface.replaced.at(-1)).toEqual([0, 2, 'insert-1']);
  });

  it('re-requests for prefixed typing events while open and closes before the replace range', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    source.resolve(0, { ...items(), from: 1 });
    await flush();
    expect(popup.isOpen()).toBe(true);

    surface.type('b', 'input.type.text');
    expect(source.calls).toHaveLength(2);
    source.resolve(1, { ...items(), from: 1 });
    await flush();
    expect(popup.isOpen()).toBe(true);

    surface.doc = surface.doc.slice(1);
    surface.anchor = 0;
    surface.head = 0;
    surface.emitChange({ docChanged: true, selectionChanged: true, userEvent: 'delete.backward', inserted: '' });
    expect(popup.isOpen()).toBe(false);
  });

  it('suppresses IME requests, invalidates pending results on composition, selection and blur', async () => {
    const composing = setup();
    composing.surface.compose(true);
    composing.surface.type('a');
    expect(composing.source.calls).toHaveLength(0);

    const during = setup();
    during.surface.type('a');
    during.surface.compose(true);
    during.source.resolve(0, items());
    await flush();
    expect(during.popup.isOpen()).toBe(false);

    const openDuringComposition = setup();
    openDuringComposition.surface.type('a');
    openDuringComposition.source.resolve(0, items());
    await flush();
    expect(openDuringComposition.popup.isOpen()).toBe(true);
    openDuringComposition.surface.compose(true);
    expect(openDuringComposition.popup.isOpen()).toBe(false);

    for (const close of [
      (fixture: ReturnType<typeof setup>) => fixture.surface.select(0),
      (fixture: ReturnType<typeof setup>) => fixture.surface.blur(),
      (fixture: ReturnType<typeof setup>) => fixture.surface.compose(true),
    ]) {
      const fixture = setup();
      fixture.surface.type('a');
      close(fixture);
      fixture.source.resolve(0, items());
      await flush();
      expect(fixture.popup.isOpen()).toBe(false);
    }
  });

  it('coalesces changes during a request into one latest request and rejects stale text', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    surface.type('b');
    surface.type('c');
    expect(source.calls).toHaveLength(1);
    source.resolve(0, items());
    await flush();
    expect(popup.isOpen()).toBe(false);
    expect(source.calls).toHaveLength(2);
    expect(source.calls[1]).toMatchObject({ text: 'abc', cursor: 3 });
    source.resolve(1, items());
    await flush();
    expect(popup.isOpen()).toBe(true);

    const stale = setup();
    stale.surface.type('a');
    stale.surface.doc = 'different';
    stale.source.resolve(0, items());
    await flush();
    expect(stale.popup.isOpen()).toBe(false);
  });

  it('closes for cursor-before-range, empty/null results, blur and selection changes', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    source.resolve(0, { ...items(), items: [] });
    await flush();
    expect(popup.isOpen()).toBe(false);

    surface.emitKey('Ctrl-Space');
    source.resolve(1, null);
    await flush();
    expect(popup.isOpen()).toBe(false);

    surface.emitKey('Ctrl-Space');
    source.resolve(2, items());
    await flush();
    surface.select(0);
    expect(popup.isOpen()).toBe(false);

    surface.emitKey('Ctrl-Space');
    source.resolve(3, items());
    await flush();
    surface.blur();
    expect(popup.isOpen()).toBe(false);

    surface.emitKey('Ctrl-Space');
    source.resolve(4, { ...items(), from: 2 });
    await flush();
    surface.select(1);
    expect(popup.isOpen()).toBe(false);

    surface.rect = null;
    surface.emitKey('Ctrl-Space');
    source.resolve(5, items());
    await flush();
    expect(popup.isOpen()).toBe(false);
  });

  it('disposes the panel and all surface subscriptions, and has no CodeMirror dependency', async () => {
    const { surface, source, popup } = setup();
    surface.type('a');
    source.resolve(0, items());
    await flush();
    expect(surface.listenerCount()).toBe(4);
    popup.dispose();
    expect(surface.listenerCount()).toBe(0);
    expect(surface.host.querySelector('.vact-completion')).toBeNull();
    popup.dispose();

    const fs = (await import(/* @vite-ignore */ nodeFsModule)) as NodeFs;
    const sourceText = fs.readFileSync(`${proc.cwd()}/src/code/completion-popup.ts`, 'utf8');
    expect(sourceText).not.toContain('@codemirror/');
  });
});
