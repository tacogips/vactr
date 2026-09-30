// What every area's `mount(root, deps)` receives (design 15.1.3).

import type { FileAccess } from '../platform/files';
import type { Client } from '../protocol/client';
import type { Store } from '../protocol/store';
import type { WasmCore } from '../protocol/wasm';
import type { BindApi, CodeApi, MidiApi, VisualApi, ResourceBudget } from './apis';
import type { Clock } from './clock';

export type Tier = 'browser' | 'native';

export interface EditorDeps {
  client: Client;
  store: Store;
  clock: Clock;
  tier: Tier;
  files: FileAccess;
  /** The browser tier's wasm core. */
  core?: WasmCore;
  resourceBudget?: ResourceBudget;
  /** Set by the owning area's mount; read at use time. */
  code?: CodeApi;
  midi?: MidiApi;
  visual?: VisualApi;
  bind?: BindApi;
}

/** The handle every `mount` returns. */
export interface Mounted {
  dispose(): void;
}

export type MountFn = (root: HTMLElement, deps: EditorDeps) => Mounted;
