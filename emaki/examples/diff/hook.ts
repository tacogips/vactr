import type { EditorHandle } from '../../src/index';
import type { DiffPair, DiffStats } from './diff-plugin';

export interface DiffHook {
  stats(): DiffStats;
  scrollTops(): [number, number];
  onApplied(cb: (sample: { rightRevision: number; presentEnd: number }) => void): () => void;
}
declare global { interface Window { __emakiDiff: DiffHook } }
export function installDiffHook(left: EditorHandle, right: EditorHandle, pair: DiffPair): () => void {
  const callbacks = new Set<(sample: { rightRevision: number; presentEnd: number }) => void>();
  let pending: { revision: number; left: number | null; right: number | null } | undefined;
  const offDecorated = pair.onDecorated(revision => { pending = { revision, left: null, right: null }; });
  const present = (side: 'left' | 'right', end: number) => {
    if (!pending) return;
    pending[side] = end;
    if (pending.left !== null && pending.right !== null) {
      const sample = { rightRevision: pending.revision, presentEnd: Math.max(pending.left, pending.right) };
      pending = undefined; for (const cb of callbacks) cb(sample);
    }
  };
  const offLeft = left.perf.onPresent(s => present('left', s.frameEnd));
  const offRight = right.perf.onPresent(s => present('right', s.frameEnd));
  window.__emakiDiff = { stats: () => pair.stats, scrollTops: () => [left.scrollTop, right.scrollTop],
    onApplied(cb) { callbacks.add(cb); return () => callbacks.delete(cb); } };
  return () => { offDecorated(); offLeft(); offRight(); callbacks.clear(); delete (window as Partial<Window>).__emakiDiff; };
}
