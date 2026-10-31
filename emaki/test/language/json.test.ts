import { describe, expect, it } from 'vitest';
import { jsonLanguage } from '../../src/language/json';
describe('JSON language', () => {
  it('identifies JSON token types and escapes', () => {
    const result = jsonLanguage.tokenizeLine('{"a": [1, true, null, "x\\"y"]}', '|v');
    expect(result.tokens.map(t => t.tag)).toEqual(['punct','key','punct','punct','number','punct','bool','punct','null','punct','string','punct','punct'].map(t => `json-${t}`));
  });
  it('carries object state between lines', () => {
    const start = jsonLanguage.tokenizeLine('{', '|v');
    const key = jsonLanguage.tokenizeLine('  "key":', start.state);
    expect(key.tokens[0].tag).toBe('json-key');
    expect(jsonLanguage.tokenizeLine('  "value"', key.state).tokens[0].tag).toBe('json-string');
  });
  it('ends unterminated strings at the line and preserves state', () => {
    const result = jsonLanguage.tokenizeLine('"unterminated', '{|k');
    expect(result.tokens).toEqual([{ from: 0, to: 13, tag: 'json-string' }]);
    expect(result.state).toBe('{|k');
  });
});
