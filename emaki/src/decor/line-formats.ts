import { MapMode, type ChangeSet, type Text } from '@codemirror/state';
import type { LineFormat, ResolvedLineFormat } from '../types';

export class LineFormatSet {
  private values: readonly LineFormat[] = [];
  constructor(private readonly length: () => number, private readonly changed: () => void = () => {}) {}
  get formats(): readonly LineFormat[] { return this.values; }
  set(formats: readonly LineFormat[]): void {
    for (const { pos } of formats) {
      if (!Number.isInteger(pos) || pos < 0 || pos > this.length()) throw new RangeError('Invalid line format position');
    }
    const freeze = (item: unknown): void => {
      if (item !== null && typeof item === 'object') { Object.values(item).forEach(freeze); Object.freeze(item); }
    };
    this.values = Object.freeze(formats.map(format => { const copy = structuredClone(format); freeze(copy); return copy; }));
    this.changed();
  }
  clear(): void { if (this.values.length) { this.values = []; this.changed(); } }
  map(changes: ChangeSet): void {
    if (changes.empty) return;
    this.values = Object.freeze(this.values.flatMap(format => {
      const pos = changes.mapPos(format.pos, -1, MapMode.TrackDel);
      return pos === null ? [] : [Object.freeze({ ...format, pos })];
    }));
    this.changed();
  }
  resolve(doc: Text): ResolvedLineFormat[] {
    return this.values.map(({ pos, style }) => ({ line: doc.lineAt(pos).number - 1, style }));
  }
}
