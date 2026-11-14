import type { JSX } from 'solid-js';

/** The stable mount point owned by Solid; CodeMirror manages its contents. */
export function CodeSurface(): JSX.Element {
  return <div class="vact-code" />;
}
