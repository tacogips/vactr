// Criterion 4 (structural): the step grid and piano roll are DISPLAYS.
// A static import scan of their sources finds no write-back module; they
// draw scripted `playing` events; pointer, keyboard and wheel events on
// them record no client envelope and leave the document unchanged; the
// roll places `:a4`, `60` and an unpitched literal in their lanes.

import { afterEach, describe, expect, it } from 'vitest';
import { ParamsArea } from '../../src/params/mount';
import { parsePitch } from '../../src/params/roll';
import type { WirePlaying } from '../../src/protocol/types';
import { cleanup, FILE, setup, spanOf, type Harness } from '../bind/fixtures';

interface Fs {
  readFileSync(path: string, enc: 'utf8'): string;
}

async function source(name: string): Promise<string> {
  const mod = 'node:fs';
  const fs = (await import(/* @vite-ignore */ mod)) as Fs;
  // vitest runs from editor/ (the package root).
  const cwd = (globalThis as unknown as { process: { cwd(): string } }).process.cwd();
  return fs.readFileSync(`${cwd}/src/params/${name}`, 'utf8');
}

const imports = (src: string): string[] => [...src.matchAll(/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g)].map((m) => m[1] as string);

const TEXT = 'd1 n [:a4 60 :bd] > s :superpiano';

let areas: ParamsArea[] = [];

afterEach(() => {
  for (const a of areas.splice(0)) a.dispose();
  cleanup();
});

function rig(): { h: Harness; params: ParamsArea } {
  const h = setup(TEXT);
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult([]);
  h.emit({ kind: 'tempo', body: { bpm: 120, beats_per_cycle: 4, cycle: [0, 1] } });
  h.transport.clear();
  return { h, params };
}

const ev = (slot: string, beat: number, literal?: string): WirePlaying => ({
  slot,
  beat: [beat, 1],
  time: beat / 2,
  dur: [1, 1],
  ...(literal ? { src: { file: FILE, span: spanOf(TEXT, literal), doc_revision: 1, form_gen: 1 } } : {}),
});

describe('step grid and piano roll are displays', () => {
  it.each(['grid.ts', 'roll.ts'])('%s imports no write-back module and registers no input handler', async (name) => {
    const src = await source(name);
    const specs = imports(src);
    expect(specs.length).toBeGreaterThan(0);
    expect(specs.filter((s) => /bind\/|protocol\/client|handles|platform\/files|\.\/(open|mount)$/.test(s))).toEqual([]);
    expect(src).not.toMatch(/addEventListener|\.dispatch\(|\.send\(|setTweak|writeSite|onclick/);
  });

  it('draws scripted playing events per slot within the current cycle', () => {
    const { h, params } = rig();
    h.emit({ kind: 'playing', body: { events: [ev('d1', 0), ev('d1', 1), ev('d2', 2), ev('d1', 3)] } });
    expect(params.grid.steps('d1').map((s) => s.pos)).toEqual([0, 0.25, 0.75]);
    const rows = [...h.root.querySelectorAll('.params-grid-row')].map((r) => [
      (r as HTMLElement).dataset.slot,
      [...r.querySelectorAll('.params-grid-step')].map((c) => (c as HTMLElement).dataset.pos),
    ]);
    expect(rows).toEqual([
      ['d1', ['0', '0.25', '0.75']],
      ['d2', ['0.5']],
    ]);
    // The next cycle replaces the slot's steps.
    h.emit({ kind: 'playing', body: { events: [ev('d1', 5)] } });
    expect(params.grid.steps('d1').map((s) => s.pos)).toEqual([0.25]);
  });

  it('pointer, keyboard and wheel events on both displays send nothing and leave the text unchanged', () => {
    const { h, params } = rig();
    h.emit({ kind: 'playing', body: { events: [ev('d1', 0, ':a4'), ev('d1', 1, '60'), ev('d1', 2, ':bd')] } });
    h.transport.clear();
    for (const tab of ['grid', 'roll'] as const) {
      params.showTab(tab);
      const el = tab === 'grid' ? params.grid.el : params.roll.el;
      const targets = [el, ...el.querySelectorAll('*')];
      expect(targets.length).toBeGreaterThan(1);
      for (const t of targets) {
        for (const type of ['pointerdown', 'pointermove', 'pointerup', 'click', 'dblclick', 'mousedown', 'mouseup']) {
          t.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: 5, clientY: 5 }));
        }
        t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
        t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Backspace', bubbles: true }));
        t.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
        t.dispatchEvent(new WheelEvent('wheel', { deltaY: 100, bubbles: true }));
      }
    }
    expect(h.transport.sent).toEqual([]);
    expect(h.text()).toBe(TEXT);
  });

  it('the roll reads pitch lanes from the source text: :a4, 60 and an unpitched literal', () => {
    const { h, params } = rig();
    h.emit({ kind: 'playing', body: { events: [ev('d1', 0, ':a4'), ev('d1', 1, '60'), ev('d1', 2, ':bd')] } });
    expect(params.roll.notes().map((n) => [n.text, n.pitch, n.pos])).toEqual([
      [':a4', 69, 0],
      ['60', 60, 0.25],
      [':bd', null, 0.5],
    ]);
    const notes = [...h.root.querySelectorAll('.params-roll-note')] as HTMLElement[];
    expect(notes.map((n) => n.dataset.pitch ?? n.dataset.lane)).toEqual(['69', '60', 'unpitched']);
    // The unpitched lane sits below every pitched lane.
    const top = (n: HTMLElement): number => parseFloat(n.style.top);
    expect(top(notes[0] as HTMLElement)).toBeLessThan(top(notes[1] as HTMLElement));
    expect(top(notes[1] as HTMLElement)).toBeLessThan(top(notes[2] as HTMLElement));
    expect(parsePitch(':c#3')).toBe(49);
    expect(parsePitch(':eb2')).toBe(39);
    expect(parsePitch('x')).toBeNull();
    expect(h.transport.sent).toEqual([]);
  });
});
