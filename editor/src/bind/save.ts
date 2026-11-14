// Mode-scoped saving (design 15.1.6 "Persistence and saving", 13).
//
// Directive mode saves the buffer text only (its `#@` comments, learned CCs
// included). ExternalFile mode saves the buffer text plus the sidecar
// `<doc>.bindings.json`. The buffer never receives overlay values; only an
// explicit commit writes a value into the text.

import type { FileAccess } from '../platform/files';
import type { EditorBindingSet, PersistenceMode } from './persistence';

export interface SaveRequest {
  files: FileAccess;
  /** The document name, e.g. `song.vact`. */
  name: string;
  /** The buffer text, exactly as edited. */
  text: string;
  mode: PersistenceMode;
  /** The ExternalFile set (overlays merged in by the caller). */
  set: EditorBindingSet;
}

export interface Saved {
  sidecar: boolean;
}

export async function save(req: SaveRequest): Promise<Saved> {
  await req.files.save(req.name, req.text);
  if (req.mode !== 'external-file') return { sidecar: false };
  await req.files.saveSidecar(req.name, req.set.toJson());
  return { sidecar: true };
}
