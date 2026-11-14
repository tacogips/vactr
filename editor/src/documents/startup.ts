import type { Draft, DraftStore, DocSource, DefaultTemplate, ExampleSource, Shell, TemplateStore } from './types';
import type { DocumentSession } from './session';

export interface StartupInput {
  shell: Shell;
  search: string;
  examples: ExampleSource;
  drafts: DraftStore | null;
  templates: TemplateStore;
  notice(message: string): void;
}

export interface StartupDocument {
  text: string;
  source: DocSource;
  restored: boolean;
  underlying?: Draft;
}

function notify(input: StartupInput, message: string): void {
  try { input.notice(message); } catch { /* Startup must not depend on a status view. */ }
}

function readWebDraft(input: StartupInput): Draft | null {
  if (input.shell !== 'web' || !input.drafts) return null;
  try { return input.drafts.readDraft(); } catch { return null; }
}

async function load(input: StartupInput, id: string, label: string): Promise<string | null> {
  if (!input.examples.find(id)) {
    notify(input, `${label} ${id} is no longer available`);
    return null;
  }
  try { return await input.examples.load(id); }
  catch {
    notify(input, `Could not load ${label} ${id}`);
    return null;
  }
}

async function templateDocument(input: StartupInput, template: DefaultTemplate): Promise<StartupDocument | null> {
  if (template.kind === 'snapshot') {
    return { text: template.text, source: { kind: 'template' }, restored: false };
  }
  const text = await load(input, template.id, 'Default template example');
  return text === null ? null : { text, source: { kind: 'example', id: template.id }, restored: false };
}

/** Resolves startup sources by precedence and absorbs storage/loader failures. */
export async function resolveStartup(input: StartupInput): Promise<StartupDocument | null> {
  try {
    const params = new URLSearchParams(input.search);
    const requestedId = params.get('example');
    if (requestedId !== null) {
      const draft = readWebDraft(input);
      if (!input.examples.find(requestedId)) {
        notify(input, `Unknown example ${requestedId}`);
      } else {
        try {
          const text = await input.examples.load(requestedId);
          return {
            text,
            source: { kind: 'example', id: requestedId },
            restored: false,
            ...(draft ? { underlying: draft } : {}),
          };
        } catch {
          notify(input, `Could not load example ${requestedId}`);
        }
      }
      if (draft) return { text: draft.text, source: draft.source, restored: true };
    }

    const draft = readWebDraft(input);
    if (draft) return { text: draft.text, source: draft.source, restored: true };

    let template: DefaultTemplate | null = null;
    try { template = await input.templates.get(); } catch { /* Broken stores fall through. */ }
    if (template) {
      const doc = await templateDocument(input, template);
      if (doc) return doc;
    }

    const fallbackId = input.examples.defaultId;
    const fallbackText = await load(input, fallbackId, 'Built-in example');
    if (fallbackText !== null) {
      return { text: fallbackText, source: { kind: 'example', id: fallbackId }, restored: false };
    }
  } catch {
    // Malformed environment adapters and URL inputs must not prevent editor startup.
  }
  notify(input, 'No startup document could be loaded');
  return null;
}

/** Applies resolved startup content without overwriting edits made during resolution. */
export function applyStartup(
  session: DocumentSession,
  doc: StartupDocument,
  notice: (message: string) => void,
): boolean {
  if (!doc.underlying) return session.loadInitial(doc.text, doc.source, { restored: doc.restored });
  if (!session.loadInitial(doc.underlying.text, doc.underlying.source, { restored: true })) return false;
  session.replace(doc.text, doc.source);
  const id = doc.source.kind === 'example' ? doc.source.id : 'document';
  try { notice(`Opened ${id}; Undo (Mod-Z) restores your unsaved draft`); } catch { /* Status is best effort. */ }
  return true;
}

