import { afterEach, describe, expect, it, vi } from 'vitest';
import { DOC_MAX_CHARS } from '../../src/documents/types';
import { TauriTemplateStore, type InvokeFn } from '../../src/documents/tauri-templates';

const makeInvoke = () =>
  vi.fn(async (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> => undefined);
const makeStore = (invoke: ReturnType<typeof makeInvoke>) =>
  new TauriTemplateStore(invoke as unknown as InvokeFn);

afterEach(() => vi.clearAllMocks());

describe('TauriTemplateStore', () => {
  it('gets a valid template through the registered command', async () => {
    const invoke = makeInvoke();
    const template = { kind: 'example', id: 'showcase' } as const;
    invoke.mockResolvedValue(template);

    await expect(makeStore(invoke).get()).resolves.toEqual(template);
    expect(invoke).toHaveBeenCalledWith('default_template_get');
  });

  it('returns null for undefined and malformed values', async () => {
    const invoke = makeInvoke();
    const templateStore = makeStore(invoke);
    invoke.mockResolvedValueOnce(undefined).mockResolvedValueOnce({ kind: 'bad' });

    await expect(templateStore.get()).resolves.toBeNull();
    await expect(templateStore.get()).resolves.toBeNull();
  });

  it('returns null when get rejects or throws synchronously', async () => {
    const rejected = makeInvoke();
    rejected.mockRejectedValue(new Error('offline'));
    await expect(makeStore(rejected).get()).resolves.toBeNull();

    const thrown = vi.fn(() => {
      throw new Error('sync failure');
    }) as unknown as InvokeFn;
    await expect(new TauriTemplateStore(thrown).get()).resolves.toBeNull();
  });

  it('sets valid templates with the expected command and arguments', async () => {
    const invoke = makeInvoke();
    const template = { kind: 'example', id: 'showcase' } as const;
    invoke.mockResolvedValue(undefined);

    await expect(makeStore(invoke).set(template)).resolves.toEqual({ ok: true });
    expect(invoke).toHaveBeenCalledWith('default_template_set', { template });
  });

  it('rejects oversized snapshots without invoking', async () => {
    const invoke = makeInvoke();
    const template = { kind: 'snapshot', name: 'Large', text: 'x'.repeat(DOC_MAX_CHARS + 1) } as const;

    await expect(makeStore(invoke).set(template)).resolves.toEqual({
      ok: false,
      reason: 'too-large',
    });
    expect(invoke).not.toHaveBeenCalled();
  });

  it('rejects other invalid templates without invoking', async () => {
    const invoke = makeInvoke();
    const template = { kind: 'snapshot', name: '', text: '' } as const;

    await expect(makeStore(invoke).set(template)).resolves.toEqual({
      ok: false,
      reason: 'invalid-name',
    });
    expect(invoke).not.toHaveBeenCalled();
  });

  it('maps set failures to an error result and does not reject', async () => {
    const rejected = makeInvoke();
    rejected.mockRejectedValue(new Error('offline'));
    await expect(
      makeStore(rejected).set({ kind: 'example', id: 'showcase' }),
    ).resolves.toEqual({ ok: false, reason: 'error', message: 'Error: offline' });

    const thrown = vi.fn(() => {
      throw new Error('sync failure');
    }) as unknown as InvokeFn;
    await expect(
      new TauriTemplateStore(thrown).set({ kind: 'example', id: 'showcase' }),
    ).resolves.toMatchObject({ ok: false, reason: 'error', message: 'Error: sync failure' });
  });

  it('resets through the registered command and maps failures', async () => {
    const invoke = makeInvoke();
    invoke.mockResolvedValue(undefined);
    await expect(makeStore(invoke).reset()).resolves.toEqual({ ok: true });
    expect(invoke).toHaveBeenCalledWith('default_template_reset');

    invoke.mockRejectedValue(new Error('offline'));
    await expect(makeStore(invoke).reset()).resolves.toEqual({
      ok: false,
      reason: 'error',
      message: 'Error: offline',
    });
  });
});
