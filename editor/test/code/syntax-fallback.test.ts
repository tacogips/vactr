import { afterEach, describe, expect, it, vi } from 'vitest';
import { EditorState } from '@codemirror/state';
import type { EditorDeps, Mounted } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import type { VactSyntax } from '../../src/code/syntax';
import { FallbackSpans } from '../../src/code/syntax';
import { tokenizerSpans } from '../../src/code/language';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const mounted: { root: HTMLElement; handle: Mounted }[] = [];

function setup(syntax?: EditorDeps['syntax']) {
  const root = document.createElement('div');
  document.body.appendChild(root);
  const layout = buildLayout(root);
  const store = new Store();
  const deps: EditorDeps = {
    client: new Client(new RecordingTransport(), { store }),
    store,
    clock: new MockClock(),
    tier: 'native',
    files: new MemoryFiles(),
    ...(syntax ? { syntax } : {}),
  };
  const handle = mount(root, deps);
  mounted.push({ root, handle });
  return { root, layout, deps, handle };
}

async function flushLoader(): Promise<void> {
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function enterCode(deps: EditorDeps): void {
  deps.code?.surface.dispatch({ changes: { from: 0, insert: 'let x 1' } });
}

afterEach(() => {
  for (const item of mounted.splice(0)) {
    item.handle.dispose();
    item.root.remove();
  }
});

describe('tree-sitter highlighting fallback', () => {
  it('keeps the tokenizer fallback active when loading rejects', async () => {
    const { layout, deps } = setup(() => Promise.reject(new Error('load failed')));
    await flushLoader();
    enterCode(deps);
    await flushLoader();
    expect(layout.code.dataset.syntax).toBe('fallback');
    expect(deps.code?.surface.state.doc.toString()).toBe('let x 1');
    expect(layout.code.textContent).not.toContain('let x 1');
    const spans = new FallbackSpans().spans(deps.code!.surface.state, 0, 7, 64).spans;
    expect(spans.map((span) => span.className)).toEqual(expect.arrayContaining(['vact-tok-head', 'vact-tok-number']));
  });

  it('switches to tree-sitter only after its loader resolves', async () => {
    const syntax: VactSyntax = {
      parse: (text) => ({
        captures: () => (text ? [{ name: 'keyword', from: 0, to: 3 }, { name: 'number', from: 6, to: 7 }] : []),
        delete: () => undefined,
      }),
      parseDoc: (doc) => ({
        captures: () => (doc.length ? [{ name: 'keyword', from: 0, to: 3 }, { name: 'number', from: 6, to: 7 }] : []),
        delete: () => undefined,
      }),
      edit: () => undefined,
    };
    const { layout, deps } = setup(() => Promise.resolve(syntax));
    expect(layout.code.dataset.syntax).toBe('fallback');
    await flushLoader();
    enterCode(deps);
    await flushLoader();
    expect(layout.code.dataset.syntax).toBe('tree-sitter');
    expect(deps.code?.surface.state.doc.toString()).toBe('let x 1');
    expect(layout.code.textContent).not.toContain('let x 1');
    const { SyntaxSpans } = await import('../../src/code/syntax');
    const spans = new SyntaxSpans(syntax).spans(deps.code!.surface.state, 0, 7, 64).spans;
    expect(spans.map((span) => span.className)).toEqual(expect.arrayContaining(['vact-tok-head', 'vact-tok-number']));
  });

  it('keeps the fallback when no loader is provided', () => {
    const { layout } = setup();
    expect(layout.code.dataset.syntax).toBe('fallback');
  });

  it('does not switch a disposed view when the loader resolves late', async () => {
    let resolve!: (syntax: VactSyntax) => void;
    const { layout, handle } = setup(() => new Promise((done) => (resolve = done)));
    handle.dispose();
    resolve({ parse: () => ({ captures: () => [], delete: () => undefined }), parseDoc: () => ({ captures: () => [], delete: () => undefined }), edit: () => undefined });
    await flushLoader();
    expect(layout.code.dataset.syntax).toBe('fallback');
  });

  it('tokenizes only the requested lines on a large document and updates after a mid-document edit', () => {
    const provider = new FallbackSpans();
    let state = EditorState.create({ doc: Array.from({ length: 20_000 }, (_, index) => `let value${index} ${index} # 日本`).join('\n') });
    const visibleFrom = state.doc.line(10_000).from;
    const base = state.doc;
    const tr = state.update({ changes: { from: base.line(10_001).from + 4, insert: 'x' } });
    provider.noteChanges(tr.changes, tr.state);
    state = tr.state;
    const visibleTo = state.doc.line(10_004).to;
    const expected = tokenizerSpans(state.doc).filter((span) => span.to > visibleFrom && span.from < visibleTo)
      .map(({ from, to, className }) => ({ from, to, className }));
    const proto = Object.getPrototypeOf(state.doc) as { toString: () => string; sliceString: (from: number, to?: number) => string };
    const stringify = vi.spyOn(proto, 'toString');
    const slices = vi.spyOn(proto, 'sliceString');
    const actual = provider.spans(state, visibleFrom, visibleTo, 10_000).spans
      .map(({ from, to, className }) => ({ from, to, className }));
    expect(actual).toEqual(expected);
    expect(stringify.mock.contexts.some((doc) => (doc as import('@codemirror/state').Text).length > 65_536)).toBe(false);
    expect(slices.mock.calls.every(([from, to]) => (to ?? Infinity) - from <= 65_536)).toBe(true);
    stringify.mockRestore();
    slices.mockRestore();
  });

  it('invalidates later fallback state when an earlier edit closes a continued string', () => {
    const provider = new FallbackSpans();
    let state = EditorState.create({ doc: 'var message "open\nlet hidden 1\nlet visible 2' });
    const before = provider.spans(state, state.doc.line(3).from, state.doc.line(3).to, 32).spans;
    const tr = state.update({ changes: { from: state.doc.line(1).to, insert: '"' } });
    provider.noteChanges(tr.changes, tr.state);
    state = tr.state;
    const line = state.doc.line(3);
    const actual = provider.spans(state, line.from, line.to, 32).spans;
    const expected = tokenizerSpans(state.doc).filter((span) => span.to > line.from && span.from < line.to)
      .map(({ from, to, className }) => ({ from, to, className }));
    expect(before.every((span) => span.className === 'vact-tok-string')).toBe(true);
    expect(actual.map(({ from, to, className }) => ({ from, to, className }))).toEqual(expected);
    expect(actual.some((span) => span.className === 'vact-tok-head')).toBe(true);
  });
});
