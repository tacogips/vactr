// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Text } from '@codemirror/state';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { InputController } from '../../src/code/input';
import { TextLayout } from '../../src/code/layout';
import { CodeViewHost } from '../../src/code/view-host';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const cleanups: Array<() => void> = [];
const twentyThousandLines = Array.from({ length: 20_000 }, (_, index) => `const value${index} = alpha beta gamma`);
function rig(source = twentyThousandLines) {
  const transport = new RecordingTransport(), store = new Store();
  const client = new Client(transport, { store, now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(source));
  const surface = new CodeSurface({ sync });
  const element = document.createElement('div'), inputContainer = document.createElement('div');
  document.body.append(element, inputContainer);
  const layout = new TextLayout({ font: '', measureText: (text) => ({ width: text.length * 8 }) },
    { font: '10px monospace', lineHeight: 20, baseline: 15 });
  layout.setText(surface.state.doc);
  const frameHost = { requestAnimationFrame: vi.fn(() => 1), cancelAnimationFrame: vi.fn(), devicePixelRatio: 1, innerHeight: 800 };
  const host = new CodeViewHost(element, surface, layout, frameHost, () => {});
  host.setViewport({ width: 800, height: 600, dpr: 1, keyboardInset: 0 });
  const input = new InputController(surface, inputContainer, {
    scrollCaret: () => host.scrollCaret(),
    onPresentation: (presentation) => layout.setText(presentation.doc, presentation.changes),
  });
  cleanups.push(() => { input.dispose(); host.dispose(); surface.dispose(); client.close(); store.dispose(); element.remove(); inputContainer.remove(); });
  return { surface, layout, host, input, element };
}
function typeBeforeInput(input: InputController, inputType: string, data: string | null): void {
  input.accessibility.textarea.dispatchEvent(new InputEvent('beforeinput', { inputType, data, bubbles: true, cancelable: true }));
}
function caretVisible(surface: CodeSurface, host: CodeViewHost, height = 600): boolean {
  const line = surface.state.doc.lineAt(surface.state.selection.main.head).number - 1;
  const top = line * 20, bottom = top + 20, scrollTop = host.scrollPosition.top;
  return top >= scrollTop && bottom <= scrollTop + height;
}
function domRect(left: number, top: number): DOMRect {
  return { x: left, y: top, left, top, right: left + 800, bottom: top + 600, width: 800, height: 600, toJSON: () => ({}) } as DOMRect;
}

afterEach(() => { for (const cleanup of cleanups.splice(0).reverse()) cleanup(); vi.restoreAllMocks(); document.body.replaceChildren(); });

describe('CodeViewHost edit-path bounds', () => {
  it('evaluates one caret per 20k-line typing transaction without shaping or rect reads', () => {
    const { surface, layout, host, input, element } = rig();
    const line = surface.state.doc.line(10_000);
    surface.dispatch({ selection: { anchor: line.from + 5 } });
    const evaluations = host.stats.caretEvaluations, builds = layout.stats.builds;
    const rect = vi.spyOn(element, 'getBoundingClientRect');
    typeBeforeInput(input, 'insertText', 'x');
    expect(host.stats.caretEvaluations - evaluations).toBe(1);
    expect(layout.stats.builds).toBe(builds);
    expect(rect).not.toHaveBeenCalled();
    expect(caretVisible(surface, host)).toBe(true);
  });

  it('keeps the caret visible at EOF for Enter and multiline paste before layout subscribers refresh', () => {
    const enter = rig(Array.from({ length: 200 }, (_, index) => `line ${index}`));
    enter.surface.dispatch({ selection: { anchor: enter.surface.state.doc.length } });
    expect(enter.host.scrollPosition.top).toBe(3_400);
    let evaluations = enter.host.stats.caretEvaluations;
    typeBeforeInput(enter.input, 'insertLineBreak', '\n');
    expect(enter.host.stats.caretEvaluations - evaluations).toBe(1);
    expect(enter.host.scrollPosition.top).toBe(3_420);
    expect(caretVisible(enter.surface, enter.host)).toBe(true);

    const paste = rig(Array.from({ length: 200 }, (_, index) => `line ${index}`));
    paste.surface.dispatch({ selection: { anchor: paste.surface.state.doc.length } });
    evaluations = paste.host.stats.caretEvaluations;
    paste.input.replaceSelection('a\nb\nc\nd\ne');
    expect(paste.host.stats.caretEvaluations - evaluations).toBe(1);
    expect(paste.host.scrollPosition.top).toBe(3_480);
    expect(caretVisible(paste.surface, paste.host)).toBe(true);
  });

  it('scrolls back when an unchanged selection was moved offscreen', () => {
    const { surface, host } = rig(Array.from({ length: 200 }, (_, index) => `line ${index}`));
    surface.dispatch({ selection: { anchor: 0 } });
    host.scrollBy(0, 5_000);
    const evaluations = host.stats.caretEvaluations;
    host.scrollCaret();
    expect(host.stats.caretEvaluations - evaluations).toBe(1);
    expect(host.scrollPosition.top).toBe(0);
  });

  it('uses the widest shaped line beyond line 1024 without shaping during clamp', () => {
    const source = Array.from({ length: 1_100 }, (_, index) => index === 1_050 ? 'x'.repeat(100) : `line ${index}`);
    const { layout, host } = rig(source);
    layout.shape(1_050);
    const builds = layout.stats.builds;
    host.scrollBy(10_000, 0);
    expect(host.scrollPosition.left).toBe(48 + 800 - 800);
    expect(layout.stats.builds).toBe(builds);
  });

  it('caches the host rect until viewport, scroll or pointer invalidation', () => {
    const { host, element } = rig(['one']);
    const rect = vi.spyOn(element, 'getBoundingClientRect').mockReturnValue(domRect(10, 20));
    host.invalidateRect();
    expect(host.viewport.left).toBe(10);
    expect(host.viewport.top).toBe(20);
    expect(rect).toHaveBeenCalledTimes(1);
    rect.mockReturnValue(domRect(30, 40));
    window.dispatchEvent(new Event('scroll'));
    expect(host.viewport.left).toBe(30);
    expect(rect).toHaveBeenCalledTimes(2);
    rect.mockReturnValue(domRect(50, 60));
    element.dispatchEvent(new Event('pointerdown'));
    expect(host.viewport.top).toBe(60);
    expect(rect).toHaveBeenCalledTimes(3);
    rect.mockReturnValue(domRect(70, 80));
    host.setViewport({ width: 800, height: 600, dpr: 1, keyboardInset: 0 });
    expect(host.viewport.left).toBe(70);
    expect(rect).toHaveBeenCalledTimes(4);
  });
});
