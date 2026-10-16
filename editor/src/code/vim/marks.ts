import type { ChangeSet } from '@codemirror/state';

export function isMarkName(ch: string): boolean {
  return /^[a-z]$/.test(ch);
}

export class MarkTable {
  private marks = new Map<string, number>();
  private contextMark: number | null = null;

  set(name: string, pos: number): void {
    this.marks.set(name, pos);
  }

  get(name: string): number | null {
    return this.marks.get(name) ?? null;
  }

  setContext(pos: number): void {
    this.contextMark = pos;
  }

  context(): number | null {
    return this.contextMark;
  }

  map(changes: ChangeSet): void {
    for (const [name, pos] of this.marks) this.marks.set(name, changes.mapPos(pos, -1));
    if (this.contextMark !== null) this.contextMark = changes.mapPos(this.contextMark, -1);
  }
}
