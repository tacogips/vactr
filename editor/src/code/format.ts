import { Transaction } from '@codemirror/state';
import type { CodeSurface } from '../app/apis';
import { minimalChange, type Formatter } from './format-core';

export { WasmFormatter, minimalChange } from './format-core';
export type { Formatter, FormatResult } from './format-core';

export const FORMAT_KEY = 'Shift-Alt-f';

export async function formatDocument(surface: CodeSurface, formatter: Formatter): Promise<boolean> {
  if (surface.compositionRange) return false;
  const before = surface.state.doc.toString();
  try {
    const result = await formatter.format(before);
    if (result.status !== 0 || result.text === before || surface.compositionRange || surface.state.doc.toString() !== before) return false;
    const change = minimalChange(before, result.text);
    if (!change) return false;
    surface.dispatch({
      changes: change,
      annotations: Transaction.userEvent.of('format'),
    });
    return true;
  } catch {
    return false;
  }
}

export function formatKeymap(surface: CodeSurface, formatter: () => Formatter | undefined): () => void {
  return surface.addKeymap([{
    key: FORMAT_KEY,
    run: () => {
      const current = formatter();
      if (!current || surface.compositionRange) return false;
      void formatDocument(surface, current);
      return true;
    },
  }], 'default');
}
