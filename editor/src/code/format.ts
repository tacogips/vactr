import type { Extension } from '@codemirror/state';
import { keymap, type EditorView } from '@codemirror/view';
import { minimalChange, type Formatter } from './format-core';

export { WasmFormatter, minimalChange } from './format-core';
export type { Formatter, FormatResult } from './format-core';

export const FORMAT_KEY = 'Shift-Alt-f';

export async function formatDocument(view: EditorView, formatter: Formatter): Promise<boolean> {
  const before = view.state.doc.toString();
  try {
    const result = await formatter.format(before);
    if (result.status !== 0 || result.text === before || view.state.doc.toString() !== before) return false;
    const change = minimalChange(before, result.text);
    if (!change) return false;
    view.dispatch({
      changes: change,
      userEvent: 'format',
    });
    return true;
  } catch {
    return false;
  }
}

export function formatKeymap(formatter: () => Formatter | undefined): Extension {
  return keymap.of([
    {
      key: FORMAT_KEY,
      run: (view) => {
        const current = formatter();
        if (!current) return false;
        void formatDocument(view, current);
        return true;
      },
    },
  ]);
}
