// The editor's panes (design 15.1.3): the transport bar on top, code on the
// left, the right pane (sliders, parameter editors, directives panel), the
// visual panes and the analyzer area, and the status line. Every pane is a
// DOM element with a stable `data-pane` attribute that areas look up.

export const PANES = ['transport', 'code', 'right', 'visual', 'analyzers', 'status'] as const;

export type PaneName = (typeof PANES)[number];

export type Layout = Record<PaneName, HTMLElement>;

/** Builds the panes under `root` (idempotent: existing panes are reused). */
export function buildLayout(root: HTMLElement): Layout {
  const doc = root.ownerDocument;
  root.classList.add('vactrol-editor');
  const get = (name: PaneName, parent: HTMLElement): HTMLElement => {
    const found = root.querySelector<HTMLElement>(`[data-pane="${name}"]`);
    if (found) return found;
    const el = doc.createElement('div');
    el.dataset.pane = name;
    el.className = `pane pane-${name}`;
    parent.appendChild(el);
    return el;
  };
  const transport = get('transport', root);
  let main = root.querySelector<HTMLElement>('.pane-main');
  if (!main) {
    main = doc.createElement('div');
    main.className = 'pane-main';
    root.appendChild(main);
  }
  const code = get('code', main);
  let side = root.querySelector<HTMLElement>('.pane-side');
  if (!side) {
    side = doc.createElement('div');
    side.className = 'pane-side';
    main.appendChild(side);
  }
  const right = get('right', side);
  const visual = get('visual', side);
  const analyzers = get('analyzers', side);
  const status = get('status', root);
  return { transport, code, right, visual, analyzers, status };
}

/** The pane `name` under `root`; throws when the layout was not built. */
export function pane(root: HTMLElement, name: PaneName): HTMLElement {
  const el = root.querySelector<HTMLElement>(`[data-pane="${name}"]`);
  if (!el) throw new Error(`no pane ${name}`);
  return el;
}
