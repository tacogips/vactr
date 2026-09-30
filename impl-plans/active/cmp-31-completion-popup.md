# CMP-31: Completion DOM Popup (`completion-popup.ts`)

**Status**: Ready
**Plan ID**: CMP-31 (wave 2; parallel with CMP-20, CMP-21, CMP-30)
**Design Reference**: `design-docs/specs/design-completion.md` 6.2, 6.3, 6.5, 7.4 (the popup bullets); user-QA C6
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan builds the popup that shows candidates while the user types.
It talks to the editor ONLY through `CompletionSurface`, and to the
engine only through `CompletionSource` (both from CMP-15). So today's
EditorView (CMP-32) and the future canvas editor can both drive it. It
must never break the eval keys, format, history or IME.

## Non-goals

- No `@codemirror/*` import, no `@codemirror/autocomplete`, and no
  stylesheet edits. The styles are minimal and inline.
- No knowledge of the wasm engine, and no timers (requests are
  event-driven).
- No edits to `mount.ts`, `language.ts` or `apis.ts`.

## Dependencies

- **dependsOn**: CMP-15
- **Blocks**: CMP-32, CMP-40

## writePaths

- `editor/src/code/completion-popup.ts` (new)
- `editor/test/code/completion-popup.test.ts` (new)
- `impl-plans/active/cmp-31-completion-popup.md` (Progress Log only)

## sharedPaths (read only)

`editor/src/code/completion-types.ts` (CMP-15). Also
`editor/src/code/transport.ts` and `editor/src/code/samples.ts`, as
examples of plain-DOM panels built with `ownerDocument.createElement`
and listener cleanup.

## Pinned Exports

```ts
export type { CompletionSurface } from './completion-types';
export class CompletionPopup {
  constructor(surface: CompletionSurface, source: CompletionSource);
  isOpen(): boolean;
  selectedIndex(): number;
  dispose(): void; // unsubscribes everything and removes the panel
}
```

DOM contract, which tests and CMP-32 rely on:

- the panel is `div.vact-completion`, `role="listbox"`, appended to
  `surface.popupHost()` only while it is open;
- each row is `div.vact-completion-item`, `role="option"`, with
  `aria-selected="true"` on the selected row;
- each row has child spans `.vact-completion-label`,
  `.vact-completion-kind` and `.vact-completion-detail`.

## Behavior (implement exactly; design 6.3)

- **Auto-trigger.** On `onChange(c)`, request when all of these hold:
  - `c.docChanged`;
  - `c.userEvent === 'input.type'`;
  - `isTriggerChar(c.inserted)`;
  - the selection is empty (`anchor === head`);
  - `!surface.isComposing()`.
- **While open.** Any `docChanged` whose `userEvent` starts with
  `input.type` or `delete` re-requests when `head >= result.from`, and
  closes otherwise.
  - A change with `userEvent === COMPLETION_USER_EVENT` never
    re-requests.
  - `selectionChanged && !docChanged` closes the popup.
- **Keys.** `onKey(k)`:
  - `Ctrl-Space`: if `source.available`, request and return `true`;
    otherwise return `false`.
  - When closed, return `false` for every other key.
  - When open:
    - arrows move the selection, wrapping;
    - `PageDown` and `PageUp` move it by 12, clamped;
    - `Enter` and `Tab` accept;
    - `Escape` closes;
    - each of these returns `true`.
- **Accept.** Call `surface.replace(result.from, head, item.insert)`,
  where `head` is the CURRENT selection head, then close.
- **Mouse.** On a row's `mousedown`, call `preventDefault()`, then
  accept that row.
- **Close.** Also close on `onBlur` and `onCompositionStart`.
  `isComposing()` also blocks showing a result that arrives mid-IME.
- **Requests.**
  - Keep an incrementing sequence number and a single in-flight flag.
  - A request made while another is in flight sets `dirty`. When the
    in-flight request settles, run exactly one follow-up request if
    `dirty`.
  - Show a result only if it is from the latest request, AND
    `surface.text()` is identical to the text the request used, AND the
    result has at least one item. Otherwise close.
  - When a result is shown, select index 0.
- **Position.** Use `surface.caretRect(result.from)`, with
  `position: fixed`, `left = rect.left` and `top = rect.bottom`. A `null`
  rect keeps the popup closed.
- **Rows.** At most 12 are visible, with `max-height` plus
  `overflow: auto`. Call `scrollIntoView` only when it exists (jsdom
  lacks it).

## Pitfalls

- Never return `true` from `onKey` while closed (except `Ctrl-Space`).
  Returning `true` would swallow Enter, Tab or Escape in the editor.
- Do not open on `input.paste`, `undo`, `redo`, `format`, `input.complete`
  or remote edits.
- Do not keep a stale result after the text changed. Compare the text,
  not only the sequence number.
- `dispose()` must remove the panel and call every unsubscribe function
  that the surface returned.

## Tests (`editor/test/code/completion-popup.test.ts`, jsdom)

Build a `FakeSurface` implementing `CompletionSurface`, with helpers
`type(ch)`, `emitKey(k)`, `select(pos)`, `blur()`, `compose(on)` and a
`replaced` log. Use a fake `CompletionSource` whose results the test
resolves by hand.

- Typing `a` -> a request is made, and after it resolves with 3 items,
  `.vact-completion` exists with 3 `role=option` rows and row 0 selected.
- Typing `:` or `.` -> a request. Typing a space -> none. `inserted` of
  `ab` (a paste) -> none. `userEvent: 'input.complete'` -> none.
- `ArrowDown` twice -> index 2. `ArrowDown` at the last row -> 0.
  `ArrowUp` from 0 -> the last row. `PageDown` -> clamped to the last row.
- `Enter` -> `replaced` equals `[from, head, insert]` for the selected
  item, the popup closes, and `onKey` returned `true`. `Tab` does the
  same.
- `Escape` closes and returns `true`.
- When closed, `Enter`, `Tab`, `Escape` and `ArrowDown` all return
  `false`.
- `Ctrl-Space` with `available` true -> a request even after a space.
  With `available` false -> returns `false`.
- Mouse: `mousedown` on row 1 -> `defaultPrevented` is true and row 1 is
  accepted.
- IME:
  - with `compose(true)`, typing `a` -> no request;
  - a result that resolves while composing -> the popup stays closed;
  - `onCompositionStart` while open -> the popup closes.
- Staleness: request A, then type again so request B is queued as dirty.
  A resolves -> not shown, B runs. A result whose text no longer matches
  -> not shown.
- A selection move without a document change -> closes. Blur -> closes.
- An empty items result -> the popup stays closed.
- `dispose()` removes the panel and unsubscribes (the fake counts
  listeners, which drop to 0).
- A source-text check: `completion-popup.ts` contains no `@codemirror/`.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`. Write logs to
`tmp/cmp/CMP-31/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `cd editor && npx tsc --noEmit`
2. `cd editor && npx vitest run test/code/completion-popup.test.ts` (a
   positive count, all passing)

## Completion Criteria

- [ ] The pinned exports and the DOM contract are implemented, and the
      behavior table is covered by the tests above.
- [ ] Keys are never consumed while the popup is closed (except
      `Ctrl-Space`), and IME and staleness rules hold.
- [ ] `completion-popup.ts` has no `@codemirror` import.
- [ ] Logs are recorded, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: Waits for CMP-15.

## Related Plans

- **Depends On**: CMP-15
- **Next**: CMP-32
