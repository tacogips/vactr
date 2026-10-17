export type ActionId = 'vactr.evaluate' | 'vactr.evaluateAll' | 'vactr.hush' | 'vactr.format'
  | 'vactr.diagnostic.next' | 'vactr.diagnostic.prev' | 'vactr.completion'
  | 'hop.char1' | 'hop.char1Line' | 'hop.word' | 'hop.line';

export const ACTION_MODES: Readonly<Record<ActionId, readonly ('n' | 'i' | 'x' | 'o')[]>> = {
  'vactr.evaluate': ['n', 'x', 'i'],
  'vactr.evaluateAll': ['n', 'x', 'i'],
  'vactr.hush': ['n', 'x', 'i'],
  'vactr.format': ['n', 'x', 'i'],
  'vactr.diagnostic.next': ['n', 'x'],
  'vactr.diagnostic.prev': ['n', 'x'],
  'vactr.completion': ['n', 'x', 'i'],
  'hop.char1': ['n', 'x', 'o'],
  'hop.char1Line': ['n', 'x', 'o'],
  'hop.word': ['n', 'x', 'o'],
  'hop.line': ['n', 'x', 'o'],
};

export function isActionId(value: string): value is ActionId {
  return Object.prototype.hasOwnProperty.call(ACTION_MODES, value);
}

export const isHopAction = (id: ActionId): boolean => id.startsWith('hop.');
