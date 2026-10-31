// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { generateJsonFixture } from '../../src/language/fixture';
describe('fixture', () => {
  it('generates a pinned 10k-line fixture', async () => {
    const text = generateJsonFixture({ lines: 10000, seed: 1 });
    expect(text.split('\n')).toHaveLength(10000); expect(JSON.parse(text)).toBeInstanceOf(Array);
    const bytes = new TextEncoder().encode(text); expect(bytes.length).toBeGreaterThanOrEqual(300000); expect(bytes.length).toBeLessThanOrEqual(600000);
    const digest = await crypto.subtle.digest('SHA-256', bytes);
    expect(Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('')).toBe('78d56a6b3cd73f3c437f83203525c73eafbbc6c579f4fd7609cf06ea4fd8d46c');
  });
  it.each([3, 10, 1000, 100000])('generates exactly %i valid lines', lines => {
    const text = generateJsonFixture({ lines }); expect(text.split('\n')).toHaveLength(lines); expect(JSON.parse(text)).toBeInstanceOf(Array);
  });
  it('rejects insufficient lines', () => expect(() => generateJsonFixture({ lines: 2 })).toThrow(RangeError));
});
