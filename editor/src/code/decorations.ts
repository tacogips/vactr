import type { AnimatedMark, Mark, MarkStyle, RenderInput, ResolvedLineFormat } from 'emaki/engine';
import type { CodeAnnotation } from '../app/apis';
import type { CursorShape } from './vim/types';
import { DEFAULT_UNAPPLIED_COLOR, unappliedRgba } from './unapplied-settings';
import type { VactrPalette } from './theme';

export interface RenderFeedback {
  annotations?: readonly CodeAnnotation[]; cursor?: number | null;
  handles?: readonly { pos: number; end?: boolean }[]; textRevision?: number; annotationsRevision?: number;
  unappliedColor?: string; animated?: readonly CodeAnnotation[]; cursorVisible?: boolean;
  cursorShape?: CursorShape; cursorEnd?: number | null;
  hopLabels?: readonly { pos: number; text: string }[]; hopRevision?: number;
  plugin?: { marks?: readonly Mark[]; animatedMarks?: readonly AnimatedMark[]; lineFormats?: readonly ResolvedLineFormat[]; key?: string };
}
export const PLAYING_MARK: MarkStyle = { background: [0.7, 0.58, 0.26, 0.5], glow: { color: [0.7, 0.58, 0.26, 0.5], radius: 4 } };
export const EVAL_MARK: MarkStyle = { background: [0.4, 0.64, 0.72, 0.5] };
export const EVAL_ERROR_MARK: MarkStyle = { background: [0.8, 0.24, 0.32, 0.5] };
export function toRenderInput(feedback: RenderFeedback, palette: VactrPalette): RenderInput {
  const tokens: NonNullable<RenderInput['tokens']>[number][] = [];
  const selection: NonNullable<RenderInput['selection']>[number][] = [];
  const composition: NonNullable<RenderInput['composition']>[number][] = [];
  const marks: Mark[] = [];
  const animatedMarks: AnimatedMark[] = [];
  for (const a of [...(feedback.annotations ?? []), ...(feedback.animated ?? [])]) {
    const range = { from: a.from, to: a.to };
    switch (a.kind) {
      case 'syntax': tokens.push({ ...range, tag: a.className ?? '' }); break;
      case 'selection': selection.push(range); break;
      case 'composition': composition.push(range); break;
      case 'unapplied': marks.push({ ...range, style: { background: unappliedRgba(feedback.unappliedColor ?? DEFAULT_UNAPPLIED_COLOR), minWidth: 6 } }); break;
      case 'diagnostic': marks.push({ ...range, style: { underline: { color: palette.diagnostic, thickness: 2 } } }); break;
      case 'call-head': marks.push({ ...range, style: { underline: { color: palette.callHead, thickness: 1 } } }); break;
      case 'binding': if (a.label) marks.push({ ...range, style: { label: { text: a.label } } }); break;
      case 'playing': animatedMarks.push({ ...range, style: PLAYING_MARK }); break;
      case 'eval': animatedMarks.push({ ...range, style: a.className?.includes('error') ? EVAL_ERROR_MARK : EVAL_MARK }); break;
      case 'momentary': if (a.label) animatedMarks.push({ ...range, style: { label: { text: a.label } } }); break;
    }
  }
  return { tokens, selection, composition, marks: [...marks, ...(feedback.plugin?.marks ?? [])],
    animatedMarks: [...animatedMarks, ...(feedback.plugin?.animatedMarks ?? [])], lineFormats: feedback.plugin?.lineFormats,
    decorationsKey: `${feedback.annotationsRevision ?? 0}|${feedback.unappliedColor ?? DEFAULT_UNAPPLIED_COLOR}${feedback.plugin?.key ?? ''}`,
    cursor: feedback.cursor, cursorShape: feedback.cursorShape, cursorEnd: feedback.cursorEnd, cursorVisible: feedback.cursorVisible,
    handles: feedback.handles, hopLabels: feedback.hopLabels, hopRevision: feedback.hopRevision, textRevision: feedback.textRevision };
}
