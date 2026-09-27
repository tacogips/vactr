// Inline SVG icons for the editor chrome (design 15.2 "Icons instead of
// words"). Every icon is decorative (`aria-hidden`); the control that holds
// it carries the accessible name and the tooltip. No icon font, no network.

import type { JSX } from 'solid-js';

interface IconProps {
  class?: string;
}

function Svg(props: IconProps & { children: JSX.Element }): JSX.Element {
  return (
    <svg
      class={`vact-icon ${props.class ?? ''}`}
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {props.children}
    </svg>
  );
}

/** Tempo. */
export const Metronome = (p: IconProps) => (
  <Svg {...p}>
    <path d="M9 3h6l4 18H5z" />
    <path d="M12 15l5-8" />
  </Svg>
);

/** Clock source: the session's own clock. */
export const ClockInternal = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v5l3 2" />
  </Svg>
);

/** Clock source: MIDI clock, locked. */
export const ClockMidiLocked = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="9" />
    <circle cx="8" cy="11" r="1" fill="currentColor" />
    <circle cx="12" cy="9" r="1" fill="currentColor" />
    <circle cx="16" cy="11" r="1" fill="currentColor" />
    <path d="M9 16h6" />
  </Svg>
);

/** Clock source: MIDI clock, lost. */
export const ClockMidiLost = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="9" />
    <path d="M8 8l8 8M16 8l-8 8" />
  </Svg>
);

/** Hush: silence everything. */
export const Hush = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 9h4l5-4v14l-5-4H4z" />
    <path d="M17 9l4 6M21 9l-4 6" />
  </Svg>
);

/** Stop / panic. */
export const StopSquare = (p: IconProps) => (
  <Svg {...p}>
    <rect x="6" y="6" width="12" height="12" rx="1" />
  </Svg>
);

/** Run the whole document. */
export const Play = (p: IconProps) => (
  <Svg {...p}>
    <path d="M7 5v14l12-7z" fill="currentColor" />
  </Svg>
);

/** Audio output running. */
export const SpeakerOn = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 9h4l5-4v14l-5-4H4z" />
    <path d="M16 9a4 4 0 0 1 0 6M18.5 6.5a8 8 0 0 1 0 11" />
  </Svg>
);

/** Audio output off / suspended. */
export const SpeakerOff = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 9h4l5-4v14l-5-4H4z" />
    <path d="M17 12h4" />
  </Svg>
);

/** Something failed. */
export const Alert = (p: IconProps) => (
  <Svg {...p}>
    <path d="M12 3l10 18H2z" />
    <path d="M12 10v5M12 18v.01" />
  </Svg>
);

/** Success. */
export const Check = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5 12l5 5 9-10" />
  </Svg>
);

/** Save the current document and binding sidecar. */
export const Save = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 3h13l3 3v15H4z" />
    <path d="M7 3v7h10V3M8 21v-8h8v8" />
  </Svg>
);

/** Load a local or remote resource into the editor. */
export const Download = (p: IconProps) => (
  <Svg {...p}>
    <path d="M12 3v13M7 11l5 5 5-5M4 19h16" />
  </Svg>
);

/** Waiting. */
export const Hourglass = (p: IconProps) => (
  <Svg {...p}>
    <path d="M7 3h10M7 21h10M8 3c0 5 8 5 8 9s-8 4-8 9M16 3c0 5-8 5-8 9s8 4 8 9" />
  </Svg>
);

/** Fold the side pane (points right, toward the edge). */
export const ChevronRight = (p: IconProps) => (
  <Svg {...p}>
    <path d="M9 6l6 6-6 6" />
  </Svg>
);

/** Unfold the side pane. */
export const ChevronLeft = (p: IconProps) => (
  <Svg {...p}>
    <path d="M15 6l-6 6 6 6" />
  </Svg>
);

/** A section that is open. */
export const ChevronDown = (p: IconProps) => (
  <Svg {...p}>
    <path d="M6 9l6 6 6-6" />
  </Svg>
);
