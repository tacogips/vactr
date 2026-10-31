export interface Hunk { aFrom: number; aTo: number; bFrom: number; bTo: number }
export interface SplitLayout {
  removed: number[];
  added: number[];
  modified: { a: number; b: number }[];
  fillers: { side: 'a' | 'b'; afterLine: number; count: number }[];
}
export { diffLines } from './lines';
export { diffWords } from './words';
export { alignSplit } from './split';
