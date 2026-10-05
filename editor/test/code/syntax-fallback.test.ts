import { afterEach, describe, expect, it } from 'vitest';
import type { EditorDeps, Mounted } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import type { VactSyntax } from '../../src/code/syntax';
import { FallbackSpans } from '../../src/code/syntax';
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
    resolve({ parse: () => ({ captures: () => [], delete: () => undefined }) });
    await flushLoader();
    expect(layout.code.dataset.syntax).toBe('fallback');
  });
});
