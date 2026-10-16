import { describe, expect, it } from 'vitest';
import {
  DOC_MAX_CHARS,
  isDefaultTemplate,
  isDocSource,
} from '../../src/documents/types';

describe('isDocSource', () => {
  it('accepts each supported source shape', () => {
    expect(isDocSource({ kind: 'example', id: 'synth-labs/lorenz' })).toBe(true);
    expect(isDocSource({ kind: 'library', id: 'x', name: 'n' })).toBe(true);
    expect(isDocSource({ kind: 'template' })).toBe(true);
    expect(isDocSource({ kind: 'new' })).toBe(true);
    expect(isDocSource({ kind: 'file', name: 'a.vact' })).toBe(true);
  });

  it('rejects malformed and unsupported source shapes', () => {
    expect(isDocSource({ kind: 'example', id: '../etc' })).toBe(false);
    expect(isDocSource({ kind: 'example', id: 'A' })).toBe(false);
    expect(isDocSource({ kind: 'library', id: 'x' })).toBe(false);
    expect(isDocSource(null)).toBe(false);
    expect(isDocSource('example')).toBe(false);
    expect(isDocSource({ kind: 'other' })).toBe(false);
  });
});

describe('isDefaultTemplate', () => {
  it('accepts a valid snapshot and example template', () => {
    expect(isDefaultTemplate({ kind: 'snapshot', name: 'My song', text: 's :fm' })).toBe(true);
    expect(isDefaultTemplate({ kind: 'example', id: 'showcase' })).toBe(true);
  });

  it('rejects invalid snapshots and unsupported template kinds', () => {
    expect(isDefaultTemplate({ kind: 'snapshot', name: '', text: '' })).toBe(false);
    expect(isDefaultTemplate({ kind: 'snapshot', name: 'x'.repeat(121), text: '' })).toBe(false);
    expect(isDefaultTemplate({ kind: 'snapshot', name: 'bad\nname', text: '' })).toBe(false);
    expect(isDefaultTemplate({ kind: 'snapshot', name: 'Large', text: 'x'.repeat(DOC_MAX_CHARS + 1) })).toBe(false);
    expect(isDefaultTemplate({ kind: 'example', id: '../etc' })).toBe(false);
    expect(isDefaultTemplate({ kind: 'other' })).toBe(false);
  });
});
