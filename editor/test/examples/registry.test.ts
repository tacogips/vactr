import { describe, expect, it } from 'vitest';
import { EXAMPLE_ID_PATTERN } from '../../src/documents/types';
import { CURATED, DEFAULT_EXAMPLE_ID, EXCLUDED } from '../../src/examples/manifest';
import { excludedIds, findExample, listExamples, loadExample, REGISTRY_FILES } from '../../src/examples/registry';

describe('example registry', () => {
  const entries = listExamples();
  const registered = new Set(entries.map(({ id }) => id));
  const excluded = new Set(excludedIds());
  const globIds = new Set(REGISTRY_FILES);

  it('partitions every glob file into exactly one registered or excluded id', () => {
    const partition = new Set([...registered, ...excluded]);
    expect([...registered].filter((id) => excluded.has(id))).toEqual([]);
    expect(partition).toEqual(globIds);
    expect(CURATED.every(({ id }) => globIds.has(id))).toBe(true);
    expect(EXCLUDED.length).toBe(6);
  });

  it('keeps ids unique and metadata within the documented bounds', () => {
    expect(registered.size).toBe(entries.length);
    for (const entry of entries) {
      expect(entry.id).toMatch(EXAMPLE_ID_PATTERN);
      expect(entry.title.length).toBeGreaterThanOrEqual(1);
      expect(entry.title.length).toBeLessThanOrEqual(60);
      expect(entry.description.length).toBeGreaterThanOrEqual(1);
      expect(entry.description.length).toBeLessThanOrEqual(140);
    }
  });

  it('registers the showcase default and marks external input requirements', () => {
    expect(DEFAULT_EXAMPLE_ID).toBe('showcase');
    expect(findExample(DEFAULT_EXAMPLE_ID)).toMatchObject({ category: 'Showcase', requires: [] });
    expect(findExample('live-external-voices')?.requires).toEqual(['audio-input']);
  });

  it('registers the template-voice examples without requirements', () => {
    const templateVoiceExamples = [
      'acid-techno',
      'bass-presets',
      'experimental-oscillators',
      'hurdy-gurdy',
      'kalimba',
      'offbeat-bass-techno',
      'rumble-techno',
      'tonewheel-organ',
      'wobble-techno',
    ];

    for (const id of templateVoiceExamples) {
      expect(findExample(id)).toMatchObject({ category: 'Examples', requires: [] });
      expect(excludedIds()).not.toContain(id);
    }
  });

  it('loads lazy raw example text and rejects excluded or unknown ids', async () => {
    await expect(loadExample('first-track')).resolves.toContain('use-bpm 124');
    await expect(loadExample('song-mode')).rejects.toThrow('unknown example song-mode');
    await expect(loadExample('sample-timestamps')).rejects.toThrow('unknown example sample-timestamps');
    await expect(loadExample('nope')).rejects.toThrow('unknown example nope');
  });

  it('generates 27 track entries from the 28-row catalog', () => {
    expect(entries.filter(({ category }) => category.startsWith('Tracks: '))).toHaveLength(27);
  });
});
