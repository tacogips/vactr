// What every area's `mount(root, deps)` receives (design 15.1.3).

import type { FileAccess } from '../platform/files';
import type { Client } from '../protocol/client';
import type { Store } from '../protocol/store';
import type { WasmCore } from '../protocol/wasm';
import type { BindApi, CodeApi, MidiApi, VisualApi, ResourceBudget, SongControls } from './apis';
import type { AudibleClock, Clock } from './clock';
import type { Formatter } from '../code/format';
import type { CompletionEngine } from '../code/completion-types';
import type { SyntaxLoader } from '../code/syntax';

export type Tier = 'browser' | 'native';

export interface EditorDeps {
  client: Client;
  store: Store;
  clock: Clock;
  audible?: AudibleClock;
  tier: Tier;
  files: FileAccess;
  /** The browser tier's wasm core. */
  core?: WasmCore;
  resourceBudget?: ResourceBudget;
  /** Optional tree-sitter syntax loader; the code area keeps StreamLanguage as fallback. */
  syntax?: SyntaxLoader;
  /** Dedicated formatter wasm instance, independent of the session core. */
  formatter?: Formatter;
  /** Optional wasm completion engine for the code area. */
  completion?: CompletionEngine;
  /** Set by the owning area's mount; read at use time. */
  code?: CodeApi;
  midi?: MidiApi;
  visual?: VisualApi;
  bind?: BindApi;
  song?: SongControls;
}

/** The handle every `mount` returns. */
export interface Mounted {
  dispose(): void;
}

export type MountFn = (root: HTMLElement, deps: EditorDeps) => Mounted;
