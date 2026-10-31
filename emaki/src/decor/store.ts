import type { ChangeSet, Text } from '@codemirror/state';
import type { AnimatedMark, Mark, ResolvedLineFormat } from '../types';
import { LineFormatSet } from './line-formats';
import { MarkSet } from './marks';

type AnimatedProvider = (frameMs: number) => readonly AnimatedMark[];
export class DecorationStore {
  private readonly markSets = new Map<string, MarkSet>();
  private readonly lineSets = new Map<string, LineFormatSet>();
  private readonly animated = new Map<object, AnimatedProvider>();
  private revision = 0;
  constructor(private readonly length: () => number) {}
  marks(owner: string): MarkSet {
    let set = this.markSets.get(owner);
    if (!set) { set = new MarkSet(this.length, () => { this.revision++; }); this.markSets.set(owner, set); }
    return set;
  }
  lineFormats(owner: string): LineFormatSet {
    let set = this.lineSets.get(owner);
    if (!set) { set = new LineFormatSet(this.length, () => { this.revision++; }); this.lineSets.set(owner, set); }
    return set;
  }
  addAnimated(provider: AnimatedProvider): () => void {
    const registration = {};
    this.animated.set(registration, provider);
    return () => { this.animated.delete(registration); };
  }
  map(changes: ChangeSet): void {
    for (const set of this.markSets.values()) set.map(changes);
    for (const set of this.lineSets.values()) set.map(changes);
  }
  collect(frameMs: number, doc: Text): { marks: Mark[]; animatedMarks: AnimatedMark[]; lineFormats: ResolvedLineFormat[]; key: string } {
    return {
      marks: Array.from(this.markSets.values()).flatMap(set => [...set.marks]),
      animatedMarks: Array.from(this.animated.values()).flatMap(provider => [...provider(frameMs)]),
      lineFormats: Array.from(this.lineSets.values()).flatMap(set => set.resolve(doc)),
      key: String(this.revision),
    };
  }
}
