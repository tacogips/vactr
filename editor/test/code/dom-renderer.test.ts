import { ChangeSet, Text } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TextLayout, type LayoutViewport } from '../../src/code/layout';
import { CanvasRenderer } from '../../src/code/renderer';
import { DomRenderer, domWindowMargin, DOM_RENDERER_LIMITS } from '../../src/code/dom-renderer';
import { selectRendererKind, type CodeRenderer } from '../../src/code/renderer-types';
import type { CodeAnnotation } from '../../src/app/apis';

const _canvasContract: CanvasRenderer extends CodeRenderer ? true : never = true;
void _canvasContract;

function setup(source: string, viewport: Partial<LayoutViewport> = {}) {
  const host = document.createElement('div');
  document.body.append(host);
  const layout = new TextLayout({ font: '', measureText: value => ({ width: value.length * 8 }) },
    { font: '14px monospace', lineHeight: 18, baseline: 14 });
  const renderer = new DomRenderer(host, layout);
  const doc = Text.of(source.split('\n'));
  renderer.setText(doc);
  renderer.setViewport({ width: 900, height: 900, scrollLeft: 0, scrollTop: 0, gutter: 48, ...viewport });
  const observer = new MutationObserver(() => {});
  observer.observe(host, { subtree: true, childList: true, attributes: true, characterData: true });
  return { host, layout, renderer, observer, doc };
}

function attachedNodes(root: HTMLElement): number {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let textNodes = 0;
  while (walker.nextNode()) textNodes++;
  return root.querySelectorAll('*').length + textNodes;
}

afterEach(() => { document.body.replaceChildren(); });

describe('DOM renderer core', () => {
  it('exports selector and structural canvas contract', () => {
    expect(selectRendererKind('')).toBe('canvas');
    expect(selectRendererKind('?renderer=dom')).toBe('dom');
    expect(selectRendererKind('?renderer=bogus')).toBe('canvas');
    expect(selectRendererKind('?renderer=dom', 'canvas')).toBe('canvas');
    expect(selectRendererKind('', 'dom')).toBe('dom');
    expect(domWindowMargin(50)).toBe(25);
    expect(domWindowMargin(1)).toBe(8);
  });

  it('virtualizes lines and uses only two transforms when scrolling within the window', () => {
    const { host, renderer, observer } = setup(Array.from({ length: 20_000 }, (_, i) => `line ${i}`).join('\n'));
    expect(renderer.render()).toBe(true);
    observer.takeRecords();
    expect(renderer.stats.liveLines).toBeLessThanOrEqual(50 + 2 * 25);
    expect(host.querySelectorAll('.vact-dom-num').length).toBeLessThanOrEqual(renderer.stats.windowTo - renderer.stats.windowFrom);
    expect(renderer.stats.peakNodes).toBeLessThan(2_000);
    const firstBuilds = renderer.stats.lineBuilds;
    renderer.setViewport({ width: 900, height: 900, scrollLeft: 0, scrollTop: 18, gutter: 48 });
    renderer.render();
    const records = observer.takeRecords();
    expect(records).toHaveLength(2);
    expect(records.every(record => record.type === 'attributes' && record.attributeName === 'style')).toBe(true);
    expect(renderer.stats.lineBuilds).toBe(firstBuilds);
    renderer.setViewport({ width: 900, height: 900, scrollLeft: 0, scrollTop: 18, gutter: 64 });
    renderer.render();
    expect(host.querySelector<HTMLDivElement>('.vact-dom-gutter')?.style.width).toBe('64px');
    expect(host.querySelector<HTMLDivElement>('.vact-dom-text')?.style.left).toBe('64px');
    for (const scrollTop of [10_000 * 18, 19_950 * 18]) {
      renderer.setViewport({ width: 900, height: 900, scrollLeft: 0, scrollTop, gutter: 64 });
      renderer.render();
      const firstLine = renderer.stats.windowFrom;
      const lastLine = renderer.stats.windowTo;
      const numbers = Array.from(host.querySelectorAll('.vact-dom-num'), node => Number(node.textContent));
      expect(renderer.stats.liveLines).toBeLessThanOrEqual(1024);
      expect(lastLine - firstLine).toBeLessThanOrEqual(1024);
      expect(numbers.every(number => number >= firstLine + 1 && number <= lastLine)).toBe(true);
    }
    expect(renderer.stats.peakNodes).toBeLessThan(2_000);
    renderer.dispose();
    expect(host.childNodes).toHaveLength(0);
    expect(renderer.stats.liveLines).toBe(0);
    expect(renderer.stats.liveNodes).toBe(0);
  });

  it('rebuilds only the changed line and reuses shifted lines after newline insertion', () => {
    const { host, renderer, observer } = setup(Array.from({ length: 300 }, (_, i) => `line ${i}`).join('\n'), { height: 90 });
    renderer.render(); observer.takeRecords();
    expect(renderer.stats.liveNodes).toBe(attachedNodes(host));
    const stableLine = Array.from(renderer.lines.children).find(node => (node as HTMLDivElement).style.transform === 'translateY(18px)');
    const before = renderer.stats.lineBuilds;
    const changed = ChangeSet.of({ from: 2, insert: 'x' }, renderer.layout.document.length);
    const edited = changed.apply(Text.of(renderer.layout.document.split('\n')));
    renderer.layout.setText(edited, changed); renderer.setText(edited); renderer.render();
    expect(renderer.stats.lineBuilds - before).toBe(1);
    expect(renderer.stats.liveNodes).toBe(attachedNodes(host));
    expect(Array.from(renderer.lines.children).find(node => (node as HTMLDivElement).style.transform === 'translateY(18px)')).toBe(stableLine);
    const editRecords = observer.takeRecords();
    expect(editRecords.filter(record => record.type === 'childList').every(record => {
      const line = (record.target as Element).closest('.vact-dom-line');
      return line === null || line === renderer.lines.querySelector('.vact-dom-line');
    })).toBe(true);
    renderer.setViewport({ width: 900, height: 90, scrollLeft: 0, scrollTop: 100 * 18, gutter: 48 });
    renderer.render(); observer.takeRecords();
    expect(renderer.stats.liveNodes).toBe(attachedNodes(host));
    const shiftedLine = Array.from(renderer.lines.children).find(node => (node as HTMLDivElement).style.transform === 'translateY(1800px)');
    expect(shiftedLine).toBeDefined();
    const current = Text.of(renderer.layout.document.split('\n'));
    const insertAt = current.line(80).from;
    const newline = ChangeSet.of({ from: insertAt, insert: '\n' }, current.length);
    const next = newline.apply(current);
    const newlineBuilds = renderer.stats.lineBuilds;
    renderer.layout.setText(next, newline); renderer.setText(next); renderer.render();
    expect(renderer.stats.lineBuilds - newlineBuilds).toBeLessThanOrEqual(1);
    expect(renderer.stats.lineMoves).toBeGreaterThan(0);
    expect((shiftedLine as HTMLDivElement).style.transform).toBe('translateY(1818px)');
    expect(renderer.stats.liveNodes).toBe(attachedNodes(host));
    renderer.dispose();
  });

  it('moves caret and selection without touching line or gutter nodes', () => {
    const { host, renderer, observer } = setup('alpha\nbeta\ngamma', { height: 54 });
    renderer.render({ cursor: 1, annotations: [{ kind: 'selection', from: 0, to: 2 }], annotationsRevision: 1 }); observer.takeRecords();
    renderer.render({ cursor: 2, annotations: [{ kind: 'selection', from: 0, to: 3 }], annotationsRevision: 2 });
    const records = observer.takeRecords();
    expect(records.some(record => (record.target as Element).closest?.('.vact-dom-caret'))).toBe(true);
    expect(records.some(record => (record.target as Element).closest?.('.vact-dom-sel'))).toBe(true);
    expect(records.some(record => (record.target as Element).closest?.('.vact-dom-lines, .vact-dom-gutter'))).toBe(false);
    expect(renderer.stats.lineBuilds).toBe(3);
    expect(host.querySelector<HTMLDivElement>('.vact-dom-caret')?.style.left).toBe('16px');
    renderer.dispose();
  });

  it('refreshes overlay geometry after an offset-preserving text revision', () => {
    const { host, layout, renderer, observer, doc } = setup(
      Array.from({ length: 300 }, (_, index) => `line ${index}`).join('\n'), { height: 90 },
    );
    const annotations: CodeAnnotation[] = [
      { kind: 'selection', from: 14, to: 18 },
      { kind: 'diagnostic', from: 14, to: 18 },
    ];
    const playing: CodeAnnotation = { kind: 'playing', from: 14, to: 18 };
    const feedback = { annotations, animated: [playing] };
    renderer.render(feedback);
    observer.takeRecords();

    const change = ChangeSet.of({ from: 13, to: 14, insert: ' ' }, doc.length);
    const edited = change.apply(doc);
    layout.setText(edited, change);
    renderer.setText(edited);
    renderer.render(feedback);

    const contentView: LayoutViewport = {
      left: 0, top: 0, width: 2 ** 24, height: 13 * 18, gutter: 0, scrollLeft: 0, scrollTop: 0,
    };
    const expected = layout.rangeRects({ from: 14, to: 18 }, contentView)[0]!;
    expect(expected).toMatchObject({ top: 18, left: 56 });
    const highlight = host.querySelector<HTMLDivElement>('.vact-dom-hlr.on');
    const selection = host.querySelector<HTMLDivElement>('.vact-dom-selr');
    const diagnostic = host.querySelector<HTMLDivElement>('.vact-dom-diag');
    expect(highlight).not.toBeNull();
    expect(selection).not.toBeNull();
    expect(diagnostic).not.toBeNull();
    expect([highlight!.style.left, highlight!.style.top]).toEqual([`${expected.left}px`, `${expected.top}px`]);
    expect([selection!.style.left, selection!.style.top]).toEqual([`${expected.left}px`, `${expected.top}px`]);
    expect([diagnostic!.style.left, diagnostic!.style.top]).toEqual([`${expected.left}px`, `${expected.bottom - 2}px`]);
    observer.takeRecords();
    renderer.render(feedback);
    expect(observer.takeRecords()).toHaveLength(0);
    renderer.dispose();
  });

  it('caps token spans and renders unknown token classes as literal plain text', () => {
    const source = 'x'.repeat(1200);
    const { host, renderer } = setup(source, { width: 10_000, height: 36 });
    const annotations: CodeAnnotation[] = Array.from({ length: 600 }, (_, index) => ({
      kind: 'syntax', from: index * 2, to: index * 2 + 1,
      className: index === 599 ? 'vact-tok-unknown' : 'vact-tok-keyword',
    }));
    renderer.render({ annotations });
    expect(host.querySelectorAll('.vact-dom-tok-keyword')).toHaveLength(512);
    expect(renderer.stats.spansMerged).toBeGreaterThan(0);
    expect(host.querySelector('.vact-dom-lines')?.textContent).toBe(source);
    renderer.dispose();
  });

  it('uses highlight class toggles for animation-only frames and bounds highlight keys', () => {
    const { host, renderer, observer } = setup('hello world');
    const { host: longHost, renderer: longRenderer, observer: longObserver } = setup('x'.repeat(800));
    const rows: CodeAnnotation[] = [0, 2, 4].map(from => ({ kind: 'playing', from, to: from + 1 }));
    renderer.render(); observer.takeRecords();
    renderer.render({ animated: rows });
    const first = observer.takeRecords();
    expect(first.length).toBeGreaterThan(0);
    expect(first.every(record => (record.target as Element).closest('.vact-dom-hl'))).toBe(true);
    expect(renderer.stats.highlightBuilds).toBeGreaterThan(0);
    expect(host.querySelectorAll('.vact-dom-hlr.on')).toHaveLength(3);
    const built = renderer.stats.highlightBuilds;
    const togglesBeforeOff = renderer.stats.highlightToggles;
    renderer.render({ animated: [] });
    const off = observer.takeRecords();
    expect(off).toHaveLength(3);
    expect(off.every(record => record.type === 'attributes' && record.attributeName === 'class' && (record.target as Element).closest('.vact-dom-hl'))).toBe(true);
    expect(renderer.stats.highlightToggles - togglesBeforeOff).toBe(3);
    expect(host.querySelectorAll('.vact-dom-hlr.on')).toHaveLength(0);
    const togglesBeforeOn = renderer.stats.highlightToggles;
    renderer.render({ animated: rows });
    const on = observer.takeRecords();
    expect(on).toHaveLength(3);
    expect(on.every(record => record.type === 'attributes' && record.attributeName === 'class' && (record.target as Element).closest('.vact-dom-hl'))).toBe(true);
    expect(renderer.stats.highlightToggles - togglesBeforeOn).toBe(3);
    expect(host.querySelectorAll('.vact-dom-hlr.on')).toHaveLength(3);
    expect(renderer.stats.highlightBuilds).toBe(built);
    const togglesBeforeRepeat = renderer.stats.highlightToggles;
    renderer.render({ animated: rows });
    expect(observer.takeRecords()).toHaveLength(0);
    expect(renderer.stats.highlightToggles - togglesBeforeRepeat).toBe(0);
    const many = Array.from({ length: 600 }, (_, from) => ({ kind: 'playing' as const, from, to: from + 1 }));
    longRenderer.render({ animated: many });
    expect(longRenderer.stats.highlightDropped).toBeGreaterThan(0);
    expect(longHost.querySelectorAll('.vact-dom-hlr').length).toBeLessThanOrEqual(DOM_RENDERER_LIMITS.maxHighlightKeys);
    expect(longHost.querySelectorAll('.vact-dom-hlr.on').length).toBe(DOM_RENDERER_LIMITS.maxHighlightKeys);
    expect(longRenderer.stats.liveNodes).toBe(attachedNodes(longHost));
    expect(DOM_RENDERER_LIMITS.maxHighlightKeys).toBe(512);
    renderer.dispose(); longObserver.takeRecords(); longRenderer.dispose();
  });

  it('renders composition, selection, literal text, syntax classes and long-line chunks', () => {
    const markup = '<b>x</b>';
    const { host, layout, renderer } = setup(`${markup}\nにほんご\n${'x'.repeat(5000)}`, { width: 160, height: 54 });
    const annotations: CodeAnnotation[] = [
      { kind: 'syntax', from: 0, to: 3, className: 'vact-tok-keyword' },
      { kind: 'selection', from: 0, to: 16 },
      { kind: 'composition', from: 8, to: 12 },
    ];
    renderer.render({ annotations, cursor: 12, textRevision: 1 });
    expect(host.querySelector('b')).toBeNull();
    expect(host.querySelector('.vact-dom-line')?.textContent).toContain(markup);
    expect(host.querySelector('.vact-dom-tok-keyword')).not.toBeNull();
    expect(host.querySelector('.vact-dom-comp')).not.toBeNull();
    expect(host.querySelectorAll('.vact-dom-selr').length).toBe(3);
    const contentView: LayoutViewport = { left: 0, top: 0, width: 2 ** 24, height: 54, gutter: 0, scrollLeft: 0, scrollTop: 0 };
    const selectionRects = layout.rangeRects({ from: 0, to: 16 }, contentView);
    expect(Array.from(host.querySelectorAll<HTMLDivElement>('.vact-dom-selr'), node => [node.style.left, node.style.top]))
      .toEqual(selectionRects.map(rect => [`${rect.left}px`, `${rect.top}px`]));
    const compositionRect = layout.rangeRects({ from: 8, to: 12 }, contentView)[0]!;
    expect(host.querySelector<HTMLDivElement>('.vact-dom-comp')?.style.left).toBe(`${compositionRect.left}px`);
    expect(host.querySelector<HTMLDivElement>('.vact-dom-comp')?.style.top).toBe(`${compositionRect.bottom - 2}px`);
    const caret = layout.coordsAtPos(12, contentView)!;
    expect(host.querySelector<HTMLDivElement>('.vact-dom-caret')?.style.left).toBe(`${caret.left}px`);
    expect(host.querySelector<HTMLDivElement>('.vact-dom-caret')?.style.top).toBe(`${caret.top}px`);
    renderer.render({ annotations: annotations.filter(row => row.kind !== 'composition'), cursor: 12, textRevision: 1 });
    expect(host.querySelectorAll('.vact-dom-comp')).toHaveLength(0);
    const longLine = layout.shape(2);
    expect(host.querySelectorAll('.vact-dom-line')[2]?.querySelectorAll('.vact-dom-run').length).toBeLessThan(layout.geometrySegments(longLine).length);
    const lineBuilds = renderer.stats.lineBuilds;
    renderer.setViewport({ width: 160, height: 54, scrollLeft: 4000, scrollTop: 0, gutter: 48 });
    renderer.render({ annotations: annotations.filter(row => row.kind !== 'composition'), cursor: 12, textRevision: 1 });
    expect(renderer.stats.lineBuilds - lineBuilds).toBe(1);
    renderer.dispose();
  });

  it('does not read layout, reports DPR, sets phase timer, and disposes idempotently', async () => {
    const bounds = vi.spyOn(Element.prototype, 'getBoundingClientRect');
    const computed = vi.spyOn(window, 'getComputedStyle');
    const { host, layout, renderer, observer } = setup('one\ntwo');
    const phases = { begin: vi.fn(), end: vi.fn() } as unknown as import('../../src/code/frame').PhaseTimer;
    renderer.setPhases(phases); expect(layout.phases).toBe(phases);
    renderer.setPhases(null); expect(layout.phases).toBeNull();
    expect(observer.takeRecords()).toHaveLength(0);
    let statuses = 0;
    const observed = new DomRenderer(document.createElement('div'), layout, { onStatus: () => statuses++ });
    observed.setViewport({ width: 100, height: 36, scrollLeft: 0, scrollTop: 0 });
    observed.render();
    const lineBuilds = observed.stats.lineBuilds;
    observed.setViewport({ width: 100, height: 36, scrollLeft: 0, scrollTop: 0 }, 2);
    observed.render();
    expect(observed.status.effectiveDpr).toBe(2);
    expect(statuses).toBe(2);
    expect(observed.stats.lineBuilds).toBe(lineBuilds);
    for (let index = 0; index < 9; index++) observed.render();
    expect(bounds).not.toHaveBeenCalled(); expect(computed).not.toHaveBeenCalled();
    const fsModule = 'node:fs/promises';
    const fs = await import(/* @vite-ignore */ fsModule) as { readFile(path: string, encoding: string): Promise<string> };
    const css = await fs.readFile('src/code/dom-renderer.css', 'utf8');
    expect(css).not.toMatch(/opacity|filter|:has\s*\(|box-shadow/i);
    expect([...css.matchAll(/border-radius\s*:\s*([^;]+);/gi)].every(match => /^\s*0(?:px)?\s*$/i.test(match[1]!))).toBe(true);
    observed.dispose(); observed.dispose();
    expect(observed.render()).toBe(false);
    expect(observed.stats.liveNodes).toBe(0);
    expect(host).toBeTruthy();
    renderer.dispose();
    bounds.mockRestore(); computed.mockRestore();
  });
});
