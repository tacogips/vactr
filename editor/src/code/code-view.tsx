import type { JSX } from 'solid-js';

/** The stable host owned by Solid; CanvasRenderer and the DOM input bridge are mounted inside. */
export function CodeSurface(): JSX.Element {
  return <div class="vact-code" />;
}
