import type { Hunk, SplitLayout } from './index';

export function alignSplit(hunks: readonly Hunk[], _aLength: number, _bLength: number): SplitLayout {
  const result: SplitLayout = { removed: [], added: [], modified: [], fillers: [] };
  for (const h of hunks) {
    const deleted = h.aTo - h.aFrom, inserted = h.bTo - h.bFrom;
    const paired = Math.min(deleted, inserted);
    for (let i = 0; i < paired; i++) result.modified.push({ a: h.aFrom + i, b: h.bFrom + i });
    for (let i = paired; i < deleted; i++) result.removed.push(h.aFrom + i);
    for (let i = paired; i < inserted; i++) result.added.push(h.bFrom + i);
    if (deleted > inserted) result.fillers.push({ side: 'b', afterLine: h.bTo - 1, count: deleted - inserted });
    if (inserted > deleted) result.fillers.push({ side: 'a', afterLine: h.aTo - 1, count: inserted - deleted });
  }
  return result;
}
