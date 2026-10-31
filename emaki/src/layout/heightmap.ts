interface BlockEntry {
  line: number;
  above: number;
  below: number;
}

/** Sparse block heights around fixed-height text rows. Lines are zero-based. */
export class Heightmap {
  private lineHeight: number;
  private lineCount = 0;
  private blocks: BlockEntry[] = [];
  private prefix = new Float64Array(1);
  private version = 0;

  constructor(lineHeight: number) {
    this.validateLineHeight(lineHeight);
    this.lineHeight = lineHeight;
  }

  get revision(): number { return this.version; }
  get blockCount(): number { return this.blocks.length; }

  setLineHeight(px: number): void {
    this.validateLineHeight(px);
    if (px !== this.lineHeight) {
      this.lineHeight = px;
      this.version++;
    }
  }

  setLineCount(count: number): void {
    if (!Number.isInteger(count) || count < 0) throw new RangeError('Invalid line count');
    if (count === this.lineCount) return;
    this.lineCount = count;
    // Without blocks every line top is `line * lineHeight`, so a line-count
    // change moves no existing line: keep the revision (geometry caches keyed
    // on it stay valid across plain edits).
    const hadBlocks = this.blocks.length > 0;
    this.blocks = this.blocks.filter((entry) => entry.line < count);
    this.rebuildPrefix();
    if (hadBlocks) this.version++;
  }

  setBlocks(entries: readonly BlockEntry[]): void {
    const merged = new Map<number, BlockEntry>();
    for (const entry of entries) {
      if (!Number.isInteger(entry.line) || entry.line < 0 ||
          !Number.isFinite(entry.above) || entry.above < 0 ||
          !Number.isFinite(entry.below) || entry.below < 0) {
        throw new RangeError('Invalid block entry');
      }
      if (entry.line >= this.lineCount) continue;
      const old = merged.get(entry.line);
      const above = (old?.above ?? 0) + entry.above;
      const below = (old?.below ?? 0) + entry.below;
      if (!Number.isFinite(above) || !Number.isFinite(below)) throw new RangeError('Invalid block height');
      merged.set(entry.line, { line: entry.line, above, below });
    }
    const next = [...merged.values()].filter((entry) => entry.above !== 0 || entry.below !== 0)
      .sort((a, b) => a.line - b.line);
    if (next.length === this.blocks.length && next.every((entry, i) => {
      const old = this.blocks[i];
      return entry.line === old.line && entry.above === old.above && entry.below === old.below;
    })) return;
    this.blocks = next;
    this.rebuildPrefix();
    this.version++;
  }

  lineTop(line: number): number {
    const index = this.lowerBound(line);
    return line * this.lineHeight + this.prefix[index] +
      (this.blocks[index]?.line === line ? this.blocks[index].above : 0);
  }

  blockAbove(line: number): number {
    const entry = this.blocks[this.lowerBound(line)];
    return entry?.line === line ? entry.above : 0;
  }

  blockBelow(line: number): number {
    const entry = this.blocks[this.lowerBound(line)];
    return entry?.line === line ? entry.below : 0;
  }

  /** Content height; `lineCount` lets callers measure the current document before `setLineCount` catches up. */
  totalHeight(lineCount = this.lineCount): number {
    const blocks = lineCount >= this.lineCount ? this.blocks.length : this.lowerBound(lineCount);
    return lineCount * this.lineHeight + this.prefix[blocks];
  }

  lineAtHeight(y: number): { line: number; region: 'text' | 'above' | 'below' } {
    if (this.lineCount === 0) return { line: 0, region: 'text' };
    if (y < 0) return { line: 0, region: this.blockAbove(0) ? 'above' : 'text' };
    if (y >= this.totalHeight()) return { line: this.lineCount - 1, region: 'text' };
    let low = 0;
    let high = this.blocks.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      const start = this.blocks[mid].line * this.lineHeight + this.prefix[mid];
      if (start <= y) low = mid + 1;
      else high = mid;
    }
    const index = low - 1;
    if (index >= 0) {
      const entry = this.blocks[index];
      const start = entry.line * this.lineHeight + this.prefix[index];
      if (y < start + entry.above) return { line: entry.line, region: 'above' };
      if (y < start + entry.above + this.lineHeight) return { line: entry.line, region: 'text' };
      if (y < start + entry.above + this.lineHeight + entry.below) return { line: entry.line, region: 'below' };
    }
    return { line: Math.floor((y - this.prefix[low]) / this.lineHeight), region: 'text' };
  }

  private lowerBound(line: number): number {
    let low = 0;
    let high = this.blocks.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (this.blocks[mid].line < line) low = mid + 1;
      else high = mid;
    }
    return low;
  }

  private rebuildPrefix(): void {
    this.prefix = new Float64Array(this.blocks.length + 1);
    for (let i = 0; i < this.blocks.length; i++) {
      this.prefix[i + 1] = this.prefix[i] + this.blocks[i].above + this.blocks[i].below;
    }
  }

  private validateLineHeight(px: number): void {
    if (!Number.isFinite(px) || px <= 0) throw new RangeError('Invalid line height');
  }
}
