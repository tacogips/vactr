// vactr's input controller: emaki's `InputController` with vactr's stop/cut
// fallback shortcut (`../ui/stop-keys`) injected by default. emaki itself
// knows no vactr shortcuts; callers may still override `fallbackShortcut`.

import { InputController as EmakiInputController, type EditorCore, type InputOptions } from 'emaki/engine';
import { fallbackStopShortcut } from '../ui/stop-keys';

export type { InputOptions, InputPresentation } from 'emaki/engine';

export class InputController extends EmakiInputController {
  constructor(surface: EditorCore, container: HTMLElement, options: InputOptions = {}) {
    super(surface, container, { fallbackShortcut: fallbackStopShortcut, ...options });
  }
}
