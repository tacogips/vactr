// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { EditorState, Text } from '@codemirror/state';
import { TextLayout, type ShapedLine, type ShapedRun } from '../../src/layout/layout';

interface NodeFs { readFileSync(path: string, encoding: 'utf8'): string }
interface NodeProcess { cwd(): string }
const proc = (globalThis as unknown as { process: NodeProcess }).process;
const nodeFsModule = 'node:fs';

const font = { font: '13px test', lineHeight: 18, baseline: 14 };
const metrics = { font: '', measureText: (text: string) => ({ width: text.length * 8 }) };
const repro = '# c\nuse-bpm 108\ns :kalimba > note :c5';
const graphemeSegmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
const graphemeMetrics = {
  font: '',
  measureText(text: string) {
    let width = 0;
    for (const part of graphemeSegmenter.segment(text)) width += part.segment.length > 1 ? 16 : 8;
    return { width };
  },
};

function layoutFor(document: string): TextLayout {
  const layout = new TextLayout({ ...metrics }, font);
  layout.setDocument(document);
  return layout;
}

function lineText(document: string, line: ShapedLine): string {
  return document.split('\n')[line.number] ?? '';
}

function cellsFor(layout: TextLayout, run: ShapedRun): string {
  return layout.textCells(run).map(cell => cell.text).join('');
}

describe('TextLayout.textCells ASCII run offsets', () => {
  it('covers every repro line and chunk run', () => {
    const layout = layoutFor(repro);
    for (let number = 0; number < 3; number++) {
      const line = layout.shape(number);
      const chunks = layout.geometrySegments(line);
      expect(chunks.flatMap(chunk => chunk.runs.map(run => cellsFor(layout, run))).join('')).toBe(lineText(repro, line));
      for (const chunk of chunks) for (const run of chunk.runs) expect(cellsFor(layout, run)).toBe(run.text);
    }
  });

  it('keeps cell x positions relative to each run', () => {
    const layout = layoutFor(repro);
    for (let number = 0; number < 3; number++) {
      const line = layout.shape(number);
      for (const chunk of layout.geometrySegments(line)) for (const run of chunk.runs) {
        for (const cell of layout.textCells(run)) {
          expect(cell.x).toBeCloseTo(layout.advance(line, run.from + cell.from) - run.x, 9);
        }
      }
    }
  });

  it('covers every line in examples/kalimba.vact', async () => {
    const fs = await import(/* @vite-ignore */ nodeFsModule) as NodeFs;
    const document = fs.readFileSync(`${proc.cwd()}/../examples/kalimba.vact`, 'utf8');
    const layout = layoutFor(document);
    for (let number = 0; number < layout.lineCount; number++) {
      const line = layout.shape(number);
      expect(line.runs.map(run => cellsFor(layout, run)).join('')).toBe(lineText(document, line));
    }
  });

  it('returns all cells for a short line at a nonzero document offset', () => {
    const layout = layoutFor('# hello\nabc def');
    const line = layout.shape(1);
    const cells = layout.textCells(line.runs[0]!);
    expect(cells).toHaveLength(7);
    expect(cells[0]).toMatchObject({ text: 'a', x: 0 });
  });

  it('starts at the run-relative visible boundary', () => {
    const layout = layoutFor(repro);
    const run = layout.shape(1).runs[0]!;
    const cells = layout.textCells(run, 2 * 8 + 1);
    expect(cells[0]).toMatchObject({ text: 'e', from: 2 });
  });

  it('keeps each tab-separated run complete', () => {
    const layout = layoutFor('x\na\tbc');
    const line = layout.shape(1);
    expect(line.runs.map(run => cellsFor(layout, run))).toEqual(line.runs.map(run => run.text));
  });

  it('preserves additive parent geometry and avoids measuring derived runs', () => {
    const layout = new TextLayout({ ...metrics }, font);
    const document = Text.of(['# first line', 'a long additive line with enough text to split into geometry chunks']);
    layout.setText(document);
    const line = layout.shape(1);
    const parent = line.runs[0]!;
    const parentCells = layout.textCells(parent);
    const expectedX = new Map(parentCells.map(cell => [cell.from, parent.x + cell.x]));
    const derivedRuns = layout.geometrySegments(line, 8).flatMap(segment => segment.runs);
    const measuredBefore = layout.stats.measuredTextCalls;

    for (const run of derivedRuns) {
      const cells = layout.textCells(run);
      expect(cells.map(cell => cell.text).join('')).toBe(run.text);
      for (const cell of cells) {
        expect(run.x + cell.x).toBeCloseTo(expectedX.get(run.from - parent.from + cell.from)!, 9);
      }
    }
    expect(layout.stats.measuredTextCalls).toBe(measuredBefore);
  });

  it('keeps derived grapheme cells aligned after a cached line shifts', () => {
    const layout = new TextLayout({ ...graphemeMetrics }, font);
    let state = EditorState.create({ doc: 'aaaa\nx👍🏽y👍🏽z' });
    layout.setText(state.doc);
    const originalLine = layout.shape(1);
    const originalRuns = originalLine.runs;
    const snapshot = (line: ShapedLine) => {
      const parentCells = line.runs.flatMap(run => layout.textCells(run).map(cell => ({
        text: cell.text, x: run.x + cell.x, width: cell.width,
      })));
      const derivedCells = layout.geometrySegments(line).flatMap(segment => segment.runs.flatMap(run => layout.textCells(run).map(cell => ({
        text: cell.text, x: run.x + cell.x, width: cell.width,
      }))));
      return { parentCells, derivedCells };
    };
    const before = snapshot(originalLine);
    expect(before.derivedCells).toEqual(before.parentCells);

    const transaction = state.update({ changes: { from: 0, insert: 'bbb' } });
    layout.setText(transaction.state.doc, transaction.changes);
    state = transaction.state;
    const shiftedLine = layout.shape(1);
    expect(shiftedLine.runs).toBe(originalRuns);
    const measuredBefore = layout.stats.measuredTextCalls;
    const after = snapshot(shiftedLine);

    expect(after.derivedCells).toEqual(before.derivedCells);
    expect(after.derivedCells).toEqual(after.parentCells);
    expect(after.derivedCells.map(cell => cell.text).join('')).toBe('x👍🏽y👍🏽z');
    expect(after.derivedCells.at(-1)).toMatchObject({ text: 'z' });
    expect(layout.stats.measuredTextCalls).toBe(measuredBefore);
  });
});
