import type { LineLanguage } from '../types';
import { tokenizeJsonLine } from './json-tokenizer';
export const jsonLanguage: LineLanguage<string> = {
  kind: 'lines', name: 'json', startState: () => '|v', tokenizeLine: tokenizeJsonLine,
  sameState: (a, b) => a === b,
};
export { generateJsonFixture } from './fixture';
