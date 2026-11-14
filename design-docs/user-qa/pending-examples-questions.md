# Example Picker and Document Persistence: Open Questions

**Status**: Pending Decision (recommendations followed by default)

**Created**: 2026-10-16 (workflow session 379, branch `wf/examples`)

**Category**: Editor

Design: [design-example-picker-and-persistence.md](../specs/design-example-picker-and-persistence.md)

## XQ1: Offer the default-template choice on the web too?

The issue asks for a default-template choice in the Tauri app and to offer
the same on the web unless the design finds a reason not to.

On the web the startup precedence is URL > draft > chosen default >
built-in default. Draft autosave writes a draft as soon as the user edits,
so on the web the chosen default only appears at startup when no draft
exists (a new browser profile, or cleared storage). It is still reachable
at any time through the picker's New action.

- Option A: offer it on the web (stored in `localStorage` under
  `vactr.doc.v1.default-template`), with New applying it.
- Option B: app only; the web always starts with the draft or the showcase.

**Recommendation**: A. It shares the picker actions and the storage module
with the app, so it costs one small store class, and New gives it a clear
use on the web.

## XQ2: Draft autosave and the named library in the Tauri app?

Requirement 4 (draft autosave, named saves) is stated for the web. In the
app the startup default template is the requested behavior; an app draft
would always take precedence over it after the first edit, so the chosen
template would almost never be used at startup. The app saves documents
through the existing file dialogs.

- Option A: app has no draft autosave and no `localStorage` library; it
  starts with URL > chosen default > built-in default and uses Open file /
  Save through dialogs.
- Option B: app also autosaves a draft (webview `localStorage`) and lists
  the library, accepting that the draft shadows the default template.

**Recommendation**: A for this release. In-session protection is the
undoable replacement plus the unsaved-change confirmation.

## XQ3: Show native-tier-only examples in the app?

`quad-stems.vact` needs a four-output host; `sample-timestamps.vact` and
`tracks/breakcore/glass-teeth.vact` need sample files that are not bundled.
The registry excludes them everywhere, so every listed example is proven by
the wasm evaluation test.

- Option A: exclude them everywhere (recorded reasons in the manifest).
- Option B: show native-only entries in the app only, which needs a
  native-tier evaluation test that this repository does not run for the
  editor.

**Recommendation**: A.
