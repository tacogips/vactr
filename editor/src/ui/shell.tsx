// The editor shell controls (design 15.2 "Foldable right pane"): a fold
// button that collapses the side column (right pane, visual panes,
// analyzers) to a narrow rail, `Mod-\` for the same, and a header per
// section that folds that section alone. Folding only hides: areas stay
// mounted, so slider state and MIDI-learned bindings survive. The states
// persist per viewer in `localStorage` (every access guarded).

import { createRoot, createSignal, type Accessor, type JSX } from 'solid-js';
import { render } from 'solid-js/web';
import { ChevronDown, ChevronLeft, ChevronRight } from './icons';

export const SIDE_KEY = 'vactrol.sideFolded';
export const SECTION_KEY = (name: string): string => `vactrol.section.${name}.folded`;

/** The side column's sections, in display order, with their labels. */
export const SECTIONS = [
  { name: 'right', label: 'controls' },
  { name: 'visual', label: 'visuals' },
  { name: 'analyzers', label: 'analyzers' },
] as const;

export type SectionName = (typeof SECTIONS)[number]['name'];

export type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;

function read(storage: StorageLike | null, key: string): boolean {
  try {
    return storage?.getItem(key) === 'true';
  } catch {
    return false;
  }
}

function write(storage: StorageLike | null, key: string, value: boolean): void {
  try {
    storage?.setItem(key, value ? 'true' : 'false');
  } catch {
    // Storage blocked (private window, preview): the state is per page load.
  }
}

/** The page's localStorage, or null when it is unavailable. */
export function pageStorage(win: Window | null): StorageLike | null {
  try {
    return win?.localStorage ?? null;
  } catch {
    return null;
  }
}

export interface Shell {
  sideFolded: Accessor<boolean>;
  toggleSide(): void;
  sectionFolded(name: SectionName): boolean;
  toggleSection(name: SectionName): void;
  dispose(): void;
}

function FoldButton(props: { folded: Accessor<boolean>; onToggle: () => void }): JSX.Element {
  const label = () => (props.folded() ? 'show the side pane (Mod-\\)' : 'fold the side pane (Mod-\\)');
  return (
    <button
      type="button"
      class="vact-icon-button vact-side-fold"
      aria-label={label()}
      title={label()}
      aria-expanded={props.folded() ? 'false' : 'true'}
      onClick={() => props.onToggle()}
    >
      {props.folded() ? <ChevronLeft /> : <ChevronRight />}
    </button>
  );
}

function SectionHeader(props: { label: string; folded: Accessor<boolean>; onToggle: () => void }): JSX.Element {
  return (
    <button
      type="button"
      class="vact-section-toggle"
      aria-expanded={props.folded() ? 'false' : 'true'}
      title={props.folded() ? `show ${props.label}` : `fold ${props.label}`}
      onClick={() => props.onToggle()}
    >
      <span class="vact-section-chevron" data-folded={props.folded() ? 'true' : 'false'}>
        <ChevronDown />
      </span>
      <span class="vact-section-label">{props.label}</span>
    </button>
  );
}

/**
 * Mounts the shell controls on a layout built by `buildLayout`: the fold
 * button in `.pane-side-bar` and a header in `.pane-section-header[data-section]`.
 */
export function mountShell(root: HTMLElement, storage: StorageLike | null): Shell {
  const doc = root.ownerDocument;
  const win = doc.defaultView;
  const bar = root.querySelector<HTMLElement>('.pane-side-bar');
  const disposers: (() => void)[] = [];
  let shell!: Shell;
  const disposeRoot = createRoot((dispose) => {
    const [sideFolded, setSideFolded] = createSignal(read(storage, SIDE_KEY));
    const sections = new Map<SectionName, { folded: Accessor<boolean>; set: (v: boolean) => void }>();
    for (const s of SECTIONS) {
      const [folded, setFolded] = createSignal(read(storage, SECTION_KEY(s.name)));
      sections.set(s.name, { folded, set: setFolded });
    }
    const apply = (): void => {
      root.dataset.sideFolded = sideFolded() ? 'true' : 'false';
      for (const s of SECTIONS) {
        const el = root.querySelector<HTMLElement>(`[data-pane="${s.name}"]`);
        if (el) el.dataset.collapsed = (sections.get(s.name) as { folded: Accessor<boolean> }).folded() ? 'true' : 'false';
      }
    };
    const toggleSide = (): void => {
      setSideFolded(!sideFolded());
      write(storage, SIDE_KEY, sideFolded());
      apply();
    };
    const toggleSection = (name: SectionName): void => {
      const s = sections.get(name);
      if (!s) return;
      s.set(!s.folded());
      write(storage, SECTION_KEY(name), s.folded());
      apply();
    };
    apply();
    if (bar) disposers.push(render(() => <FoldButton folded={sideFolded} onToggle={toggleSide} />, bar));
    for (const s of SECTIONS) {
      const header = root.querySelector<HTMLElement>(`.pane-section-header[data-section="${s.name}"]`);
      const state = sections.get(s.name) as { folded: Accessor<boolean> };
      if (header) disposers.push(render(() => <SectionHeader label={s.label} folded={state.folded} onToggle={() => toggleSection(s.name)} />, header));
    }
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === '\\' && (e.metaKey || e.ctrlKey) && !e.altKey) {
        e.preventDefault();
        toggleSide();
      }
    };
    win?.addEventListener('keydown', onKey);
    disposers.push(() => win?.removeEventListener('keydown', onKey));
    shell = {
      sideFolded,
      toggleSide,
      sectionFolded: (name) => sections.get(name)?.folded() ?? false,
      toggleSection,
      dispose: () => undefined,
    };
    return dispose;
  });
  return {
    ...shell,
    dispose() {
      for (const d of disposers.reverse()) d();
      disposeRoot();
    },
  };
}

/** Section labels (tests, tooltips). */
export function sectionLabels(): string[] {
  return SECTIONS.map((s) => s.label);
}

