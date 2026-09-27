import { For, type Accessor, type JSX } from 'solid-js';
import type { Tier } from '../app/deps';
import { Download } from '../ui/icons';
import type { UnresolvedImport } from './driver';

export interface PaneDiagView { code: string; message: string }
export interface PkgModel {
  unresolved: UnresolvedImport[];
  diagnostics: PaneDiagView[];
  busy: boolean;
  status: string;
  hint: string;
}

export function PkgView(props: {
  tier: Tier;
  proxy: string;
  model: Accessor<PkgModel>;
  onProxy: (value: string) => void;
  onImport: (path: string) => void;
}): JSX.Element {
  return <section class="pkg-pane" data-area="pkg">
    <div class="pkg-title">Packages</div>
    <label class="pkg-proxy" hidden={props.tier !== 'browser'}>Proxy{' '}
      <input type="url" placeholder="https://proxy.example" data-pkg="proxy" value={props.proxy}
        on:change={(event) => props.onProxy(event.currentTarget.value.trim())} />
    </label>
    <div class="pkg-hint" data-pkg="hint" hidden={props.model().hint === ''}>{props.model().hint}</div>
    <ul class="pkg-imports" data-pkg="imports"><For each={props.model().unresolved}>{(item) =>
      <li data-pkg-path={item.path} title={item.message}>
        <span class="pkg-path">{item.path}</span>
        {props.tier === 'native' ?
          <code data-pkg="get-command">{`vactrol get ${item.path}`}</code> :
          <button type="button" class="vact-icon-button" data-pkg-action="import" disabled={props.model().busy}
            aria-label={`Import ${item.path}`} title={`Import ${item.path}`} on:click={() => props.onImport(item.path)}><Download /></button>}
      </li>
    }</For></ul>
    <div class="pkg-status" data-pkg="status">{props.model().status}</div>
    <ul class="pkg-diagnostics" data-pkg="diagnostics"><For each={props.model().diagnostics}>{(diag) =>
      <li data-code={diag.code}>{`${diag.code}: ${diag.message}`}</li>
    }</For></ul>
  </section>;
}
