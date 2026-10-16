import {
  DOC_MAX_CHARS,
  isDefaultTemplate,
  type DefaultTemplate,
  type StorageResult,
  type TemplateStore,
} from './types';

export type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

function isOversizedSnapshot(value: unknown): boolean {
  if (typeof value !== 'object' || value === null || !('kind' in value) || !('text' in value)) {
    return false;
  }
  return value.kind === 'snapshot' && typeof value.text === 'string' && value.text.length > DOC_MAX_CHARS;
}

export class TauriTemplateStore implements TemplateStore {
  constructor(private readonly invoke: InvokeFn) {}

  async get(): Promise<DefaultTemplate | null> {
    try {
      const value: unknown = await this.invoke('default_template_get');
      return isDefaultTemplate(value) ? value : null;
    } catch {
      return null;
    }
  }

  async set(template: DefaultTemplate): Promise<StorageResult> {
    if (!isDefaultTemplate(template)) {
      return {
        ok: false,
        reason: isOversizedSnapshot(template) ? 'too-large' : 'invalid-name',
      };
    }
    try {
      await this.invoke('default_template_set', { template });
      return { ok: true };
    } catch (error) {
      return { ok: false, reason: 'error', message: String(error) };
    }
  }

  async reset(): Promise<StorageResult> {
    try {
      await this.invoke('default_template_reset');
      return { ok: true };
    } catch (error) {
      return { ok: false, reason: 'error', message: String(error) };
    }
  }
}
