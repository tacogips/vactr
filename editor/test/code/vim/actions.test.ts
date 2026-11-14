import { describe, expect, it } from 'vitest';
import { ACTION_MODES, isActionId, isHopAction, type ActionId } from '../../../src/code/vim/actions';

describe('action table', () => {
  it('matches the design mode registry exactly', () => expect(ACTION_MODES).toEqual({
    'vactr.evaluate': ['n', 'x', 'i'], 'vactr.evaluateAll': ['n', 'x', 'i'], 'vactr.hush': ['n', 'x', 'i'],
    'vactr.format': ['n', 'x', 'i'], 'vactr.diagnostic.next': ['n', 'x'], 'vactr.diagnostic.prev': ['n', 'x'],
    'vactr.completion': ['n', 'x', 'i'], 'hop.char1': ['n', 'x', 'o'], 'hop.char1Line': ['n', 'x', 'o'],
    'hop.word': ['n', 'x', 'o'], 'hop.line': ['n', 'x', 'o'],
  }));
  it.each(Object.entries(ACTION_MODES) as [ActionId, readonly ('n' | 'i' | 'x' | 'o')[]][])('exports modes for %s', (id, modes) => {
    expect(isActionId(id)).toBe(true);
    expect(modes.length).toBeGreaterThan(0);
    expect([...modes].every((mode) => ['n', 'i', 'x', 'o'].includes(mode))).toBe(true);
  });
  it.each(['vactr.evaluate', 'vactr.evaluateAll', 'vactr.hush', 'vactr.format', 'vactr.diagnostic.next', 'vactr.diagnostic.prev', 'vactr.completion'])('does not classify %s as hop', (id) => expect(isHopAction(id as ActionId)).toBe(false));
  it.each(['hop.char1', 'hop.char1Line', 'hop.word', 'hop.line'])('classifies %s as hop', (id) => expect(isHopAction(id as ActionId)).toBe(true));
  it('rejects unknown action ids', () => expect(isActionId('nope')).toBe(false));
});
