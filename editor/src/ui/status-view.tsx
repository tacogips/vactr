import type { JSX } from 'solid-js';

export function BootFailure(props: { reason: string }): JSX.Element {
  return <div data-state="failed" role="alert">{`failed to start: ${props.reason}`}</div>;
}
