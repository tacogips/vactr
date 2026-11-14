import { EditorState, Text } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import type { CodeSurface, CodeSurfaceUpdate } from '../../src/app/apis';
import { EditorSettingsSource, mediaUrl, parseEditorSettings } from '../../src/visual/settings';

const comment = (value: unknown) => `#@ editor ${JSON.stringify(value)}`;
function surface(source: string) {
  let state = EditorState.create({ doc: source });
  const listeners = new Set<(update: CodeSurfaceUpdate) => void>();
  return {
    api: { get state() { return state; }, subscribe(cb: (u: CodeSurfaceUpdate) => void) { listeners.add(cb); return () => listeners.delete(cb); } } as unknown as CodeSurface,
    edit(from: number, to: number, insert: string) {
      const tr = state.update({ changes: { from, to, insert } }); state = tr.state;
      for (const cb of listeners) cb({ state, changes: tr.changes, docChanged: true, selectionSet: false, userEvent: null });
      return state.doc;
    },
    get listeners() { return listeners.size; },
  };
}

describe('editor presentation comments', () => {
  it('merges own-line settings and ignores strings, trailing comments and other directive names', () => {
    const parsed = parseEditorSettings([
      'text "#@ editor {bad}" > out',
      'let x 1 #@ editor {bad}',
      '# ordinary #@ editor {bad}',
      '#@ editorial lpf cc: 1',
      comment({ background: { kind: 'image', src: '/cover with spaces.png', opacity: 0.7 } }),
      `  ${comment({ visualizers: [{ kind: 'oscilloscope' }, { kind: 'spectrum', position: 'top' }] })}`,
    ].join('\n'));
    expect(parsed.errors).toEqual([]);
    expect(parsed.settings.background).toMatchObject({ kind: 'image', src: '/cover with spaces.png', opacity: 0.7, scrim: 0.55 });
    expect(parsed.settings.visualizers.map((v) => v.kind)).toEqual(['oscilloscope', 'spectrum']);
    const changed = parseEditorSettings(`${comment({ background: { kind: 'video', src: '/a.mp4', blur: 5 } })}\n${comment({ background: { kind: 'none' } })}`);
    expect(changed.settings.background).toMatchObject({ kind: 'none', blur: 0 });
    expect(changed.settings.background?.src).toBeUndefined();
  });

  it('reports line-specific malformed, unknown, unsafe and out-of-range settings without partially applying a line', () => {
    const parsed = parseEditorSettings([
      comment({ background: { kind: 'image', src: '/valid.png' } }),
      '#@ editor {',
      comment({ background: { kind: 'video', src: '/replace.mp4' }, visualizers: [{ kind: 'fake' }] }),
      comment({ background: { kind: 'image', src: 'javascript:alert(1)' } }),
      comment({ background: { kind: 'image', src: 'data:text/html,<script>x</script>' } }),
      comment({ visualizers: [{ kind: 'oscilloscope', opacity: 2 }] }),
      comment({ background: { kind: 'shader', output: 4 } }),
      comment({ unknown: true }),
    ].join('\n'));
    expect(parsed.errors).toHaveLength(7);
    expect(parsed.errors[0]).toMatch(/^Line 2:/);
    expect(parsed.settings.background?.src).toBe('/valid.png');
    expect(parsed.settings.visualizers).toEqual([]);
    expect(mediaUrl('./image.png', 'image', 'https://example.test/editor/')).toBe('https://example.test/editor/image.png');
    expect(() => mediaUrl('file:///private/a.png', 'image', 'https://example.test/')).toThrow('scheme');
  });

  it('tracks replacement, deletion, inserted directive blocks, undo-style restoration and disposal', () => {
    const first = comment({ background: { kind: 'image', src: '/first.png' } });
    const f = surface(`${first}\nlet x 1`);
    const changes = vi.fn();
    const source = new EditorSettingsSource(f.api, changes);
    expect(changes.mock.lastCall?.[0].settings.background.src).toBe('/first.png');
    f.edit(0, first.length + 1, '');
    expect(changes.mock.lastCall?.[0].settings).toEqual({ background: null, visualizers: [] });
    f.edit(0, 0, `${first}\n`);
    expect(changes.mock.lastCall?.[0].settings.background.src).toBe('/first.png');
    const block = `\n${comment({ visualizers: [{ kind: 'spectrogram' }] })}\n${comment({ background: { kind: 'video', src: '/loop.webm' } })}`;
    f.edit(f.api.state.doc.length, f.api.state.doc.length, block);
    expect(changes.mock.lastCall?.[0].settings.background.kind).toBe('video');
    expect(changes.mock.lastCall?.[0].settings.visualizers[0].kind).toBe('spectrogram');
    source.dispose(); expect(f.listeners).toBe(0);
  });

  it('does not rescan the document for ordinary code edits and updates error line numbers after insertion', () => {
    const f = surface(`${'# plain\n'.repeat(20_000)}let value 1`);
    const changes = vi.fn();
    const source = new EditorSettingsSource(f.api, changes);
    const calls = vi.spyOn(Text.prototype, 'iterLines');
    f.edit(f.api.state.doc.length, f.api.state.doc.length, '0');
    expect(calls.mock.calls.every((args) => args.length > 0)).toBe(true);
    expect(changes).toHaveBeenCalledOnce();
    calls.mockRestore();
    f.edit(0, 0, '#@ editor {\n');
    expect(changes.mock.lastCall?.[0].errors[0]).toMatch(/^Line 1:/);
    f.edit(0, 0, '# ordinary\n');
    expect(changes.mock.lastCall?.[0].errors[0]).toMatch(/^Line 2:/);
    source.dispose();
  });
});
