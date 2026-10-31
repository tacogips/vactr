import type { ChangeSet } from '@codemirror/state';
import type { Mark } from '../types';

function freezeCopy<T>(value: T): T {
  const copy = structuredClone(value);
  const freeze = (item: unknown): void => {
    if (item !== null && typeof item === 'object') {
      Object.values(item).forEach(freeze);
      Object.freeze(item);
    }
  };
  freeze(copy);
  return copy;
}

export class MarkSet {
  private values: readonly Mark[] = [];
  constructor(private readonly length: () => number, private readonly changed: () => void = () => {}) {}
  get marks(): readonly Mark[] { return this.values; }
  set(marks: readonly Mark[]): void {
    for (const { from, to } of marks) {
      if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || from > to || to > this.length()) throw new RangeError('Invalid mark range');
    }
    this.values = Object.freeze(marks.map(freezeCopy));
    this.changed();
  }
  clear(): void { if (this.values.length) { this.values = []; this.changed(); } }
  map(changes: ChangeSet): void {
    if (changes.empty) return;
    this.values = Object.freeze(this.values.flatMap(mark => {
      const from = changes.mapPos(mark.from, 1);
      const to = changes.mapPos(mark.to, -1);
      if (from >= to && mark.style.minWidth === undefined) return [];
      return [Object.freeze({ ...mark, from, to: Math.max(from, to) })];
    }));
    this.changed();
  }
}
