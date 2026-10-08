import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CodeSurface, MomentaryGesture } from '../../src/app/apis';
import { MomentaryPointer } from '../../src/code/momentary-pointer';

const event = (name: string, props: Record<string, unknown> = {}): Event => Object.assign(new Event(name, { bubbles: true, cancelable: true }), props);
const setup = () => {
  const el = document.createElement('canvas'); document.body.append(el);
  let now = 1000;
  const gesture: MomentaryGesture = { move: vi.fn(), end: vi.fn() };
  const surface = {
    posAtCoords: ({ x }: { x: number }) => Math.round(x),
    momentaryHit: (pos: number) => pos < 20,
    momentaryDrag: vi.fn(() => gesture),
  } as unknown as CodeSurface;
  const cancelSelection = vi.fn();
  const pointer = new MomentaryPointer(surface, el, { now: () => now, cancelSelection });
  return { el, gesture, surface, cancelSelection, pointer, setNow: (v: number) => { now = v; } };
};
afterEach(() => document.body.replaceChildren());

describe('momentary pointer routing', () => {
  it('routes right drag, Shift release and only suppresses context menus on site/recent gesture', () => {
    const h = setup();
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'mouse', pointerId: 3, button: 2, buttons: 2, clientX: 5, clientY: 90 }));
    h.el.dispatchEvent(event('pointermove', { pointerType: 'mouse', pointerId: 3, clientY: 50 }));
    const up = event('pointerup', { pointerType: 'mouse', pointerId: 3, button: 2, shiftKey: true }); h.el.dispatchEvent(up);
    expect(h.surface.momentaryDrag).toHaveBeenCalledWith({ pos: 5, clientY: 90, kind: 'mouse' });
    expect(h.gesture.move).toHaveBeenCalledWith(50); expect(h.gesture.end).toHaveBeenCalledWith(true);
    const outside = event('contextmenu', { clientX: 50, clientY: 1 }); h.el.dispatchEvent(outside);
    expect(outside.defaultPrevented).toBe(true); // recent right gesture
    h.setNow(1601);
    const menu = event('contextmenu', { clientX: 50, clientY: 1 }); h.el.dispatchEvent(menu);
    expect(menu.defaultPrevented).toBe(false);
    const siteMenu = event('contextmenu', { clientX: 5, clientY: 1 }); h.el.dispatchEvent(siteMenu);
    expect(siteMenu.defaultPrevented).toBe(true);
    h.pointer.dispose();
  });

  it('does not start a chorded right press during a left drag', () => {
    const h = setup();
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'mouse', pointerId: 1, button: 0, buttons: 1, clientX: 5, clientY: 20 }));
    h.el.dispatchEvent(event('pointermove', { pointerType: 'mouse', pointerId: 1, button: 0, buttons: 3, clientX: 8, clientY: 18 }));
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'mouse', pointerId: 2, button: 2, buttons: 3, clientX: 5, clientY: 20 }));
    expect(h.surface.momentaryDrag).not.toHaveBeenCalled();
    h.pointer.dispose();
  });

  it('pairs two touch pointers by centroid and ends when either lifts', () => {
    const h = setup();
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 4, button: 0, clientX: 5, clientY: 20 }));
    h.setNow(1100);
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 5, button: 0, clientX: 9, clientY: 40 }));
    expect(h.surface.momentaryDrag).toHaveBeenCalledWith({ pos: 5, clientY: 30, kind: 'touch' });
    expect(h.cancelSelection).toHaveBeenCalledOnce();
    h.el.dispatchEvent(event('pointermove', { pointerType: 'touch', pointerId: 5, clientY: 60 }));
    expect(h.gesture.move).toHaveBeenCalledWith(40);
    h.el.dispatchEvent(event('pointerup', { pointerType: 'touch', pointerId: 5, shiftKey: false }));
    expect(h.gesture.end).toHaveBeenCalledWith(false);
    h.pointer.dispose();
  });

  it('keeps independent touch pairs active at the same time', () => {
    const h = setup();
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 10, button: 0, clientX: 1, clientY: 10 }));
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 11, button: 0, clientX: 10, clientY: 30 }));
    h.setNow(1100);
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 12, button: 0, clientX: 2, clientY: 20 }));
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 13, button: 0, clientX: 11, clientY: 40 }));
    expect(h.surface.momentaryDrag).toHaveBeenCalledTimes(2);
    h.el.dispatchEvent(event('pointerup', { pointerType: 'touch', pointerId: 10 }));
    h.el.dispatchEvent(event('pointerup', { pointerType: 'touch', pointerId: 12 }));
    h.el.dispatchEvent(event('pointerup', { pointerType: 'touch', pointerId: 13 }));
    expect(h.gesture.end).toHaveBeenCalledTimes(2);
    h.pointer.dispose();
  });

  it('does not pair touches when the second down is outside 250 ms', () => {
    const h = setup();
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 20, button: 0, clientX: 5, clientY: 20 }));
    h.setNow(1300);
    h.el.dispatchEvent(event('pointerdown', { pointerType: 'touch', pointerId: 21, button: 0, clientX: 8, clientY: 40 }));
    expect(h.surface.momentaryDrag).not.toHaveBeenCalled();
    h.pointer.dispose();
  });
});
