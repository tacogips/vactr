# Command Design

This document specifies the `vactr` command-line interface and the
editor-runtime session protocol on the wire. The behavior rules behind
them (the eval pipeline, reactive publication, write authority,
packages, directives, the socket's security rules) are in
`design-implementation.md` sections 5.7, 13.5, 14 and 14.5.

## Overview

`vactr` is a single binary. The `lsp` verb needs the `lsp` cargo
feature; every other verb is in the default build (`host-native`).
Every verb runs one `Session` on the main thread (design 14.5.4).

---

## Sections

### Naming (decided 2026-09-24)

| Item | Value |
|------|-------|
| Language | Vactr (inspired by an LED coupled to a photoresistor: light controlling sound) |
| Binary and crate | `vactr` |
| Source file extension | `.vact` |
| Repository | https://github.com/tacogips/vactr |

### Subcommands

| Verb | Synopsis | Behavior |
|------|----------|----------|
| `repl` | `vactr repl [--host H] [--audio-in]` | Interactive console over one session (design 14.2, 14.5.10). Sound keeps playing between lines. Completed expressions bind `_1`, `_2`, …; a failed one binds nothing. EOF (Ctrl-D) exits. |
| `run` | `vactr run <file.vact> [--host H] [--audio-in] [--cycles N]` | Evaluates the whole document once (packages from `vactr.lock` + cache). Prints diagnostics and console output. A finite Song completes automatically after its tail and ownership cleanup; legacy playback ticks until interrupted or for N cycles when `--cycles` is given. |
| `render` | `vactr render <file.vact> <output.wav> [--sample-rate HZ]` | Builds an isolated finite Song and exports the complete arrangement and declared tail as stereo PCM16 WAV. Requires `host-native`; no output device or manual cycle cap is needed. |
| `serve` | `vactr serve [<file.vact>] [--host H] [--audio-in] [--port P] [--bind 127.0.0.1]` | Starts the session socket, optionally evaluating a file first, and prints the connect URL with the token once to stderr. Serves until interrupted. |
| `get` | `vactr get [<package-path>[@<version>]] [--store dir:<root>]` | With a path: adds or raises the requirement in `./vactr.toml` (created with only `[deps]` if absent). Then, and also with no path: runs minimal version selection, fetches and verifies every selected package into the cache, and writes `./vactr.lock`. The default store is git; `--store dir:<root>` uses a local-directory store (`<root>/<path>@<version>/`). |
| `lsp` | `vactr lsp [--session <ws-url>]` | Language server over stdio (design 14.3, 14.5.11). With `--session` it attaches to a running `serve` socket for runtime diagnostics. A build without the `lsp` feature prints an error and exits 1. |
| `fmt` | `vactr fmt [--check] <PATH>...` or `vactr fmt [--check] -` | Formats `.vact` sources, rewriting each changed file in place. With `-`, it reads stdin and writes stdout. `--check` writes nothing and prints each path that would change. An input with reader errors is left unchanged, and its diagnostics go to stderr. The one exception is a file whose only defect is unambiguous space indentation: it is converted to tabs (section 3.9). The rules are in `design-formatter-and-syntax.md` section 3. |
| (none) | `vactr --version`, `vactr help` | Prints the version or the usage. |

A package path is `github.com/<owner>/<name>` (lowercase), and a
version is a git tag `vMAJOR.MINOR.PATCH[-pre]`. With no version given,
`get` picks the highest non-prerelease tag.

### Flags and Options

| Flag | Verbs | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--host` | repl, run, serve | `native` \| `noop` | `native` | Audio/MIDI host. If the native device fails to open, the CLI warns on stderr and falls back to `noop`. |
| `--audio-in` | repl, run, serve | switch | off | Request the default f32 input device at the output sample rate. Requires native host; a missing or incompatible input exits 1 rather than falling back to `noop`. Mono is duplicated, stereo is preserved, and wider input uses its first two channels. A bounded callback queue silences underruns and trims stale backlog; independent device clocks can drift. |
| `--cycles` | run | positive int | (run until interrupted) | Stop after N cycles. With `--host noop` a virtual clock is used, so the run ends immediately and deterministically. |
| `--sample-rate` | render | integer, 8000–192000 | `48000` | Output sample rate in Hz. |
| `--port` | serve | u16 | `0` (OS-assigned) | TCP port. The chosen port is printed in the connect URL. |
| `--bind` | serve | IP address | `127.0.0.1` | Only loopback addresses are accepted; anything else is a usage error. |
| `--store` | get | `dir:<path>` | (git) | Resolve and fetch from a local-directory store instead of git. |
| `--session` | lsp | ws URL with token | (none) | Attach to a live session socket. |
| `--check` | fmt | switch | off | Report without writing: print each path that would change, and exit 1 if any would. |

### Finite song playback and export

Functions may return immutable Parts that are edited, repeated and sequenced
before `song ... > play-song`. The self-contained canonical example has 24
cycles at 120 BPM with four beats per cycle and an eight-second tail:

```bash
vactr run examples/song-mode.vact --host native
vactr render examples/song-mode.vact song.wav --sample-rate 48000
```

At 48 kHz its complete export contains 2,688,000 stereo frames (56 seconds).
Render reports frame count, sample rate, revision, epoch, seed and final state;
it does not write a separate metadata file. Candidate/export failures print
their diagnostic and exit 3; source read failures exit 1.

See [song-mode design](design-song-mode.md) for timing, ownership and editing
contracts. Native execution and the full canonical WAV were verified on
2026-10-03; these examples do not establish completion of remaining Index
routing support or the full regression gates.

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `VACTR_HOME` | No | `~/.vactr` | Root for the package cache `$VACTR_HOME/pkg/<path>@<version>` and its staging area `$VACTR_HOME/pkg/.staging/`. |
| `HOME` | Yes when `VACTR_HOME` is unset | (OS) | Used to derive the default `VACTR_HOME`. |

The git store runs `git` with `GIT_TERMINAL_PROMPT=0` and
`GIT_CONFIG_NOSYSTEM=1` set for that child process only. No verb reads
or stores secrets. The session token exists only in memory and in the
printed URL.

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success. For `run`, the document evaluated with no error-severity diagnostic and no failed form. |
| 1 | General error: IO, audio host, package resolution or integrity (`get`), socket bind. For `fmt --check`: no IO error and no refused input, but at least one input would change. |
| 2 | Usage error: unknown verb or flag, missing argument, non-loopback `--bind`, `fmt` with no input or with `-` mixed with paths. |
| 3 | `run`: the document reported error-severity diagnostics or failed forms. The run still played for the requested cycles and printed everything. `fmt`: no IO error, but at least one input had reader errors and was left unchanged. |

### Files

| File | Written by | Format |
|------|------------|--------|
| `vactr.toml` | the user; `get` adds `[deps]` entries | TOML subset: `[package]` (`path`, `vactr`, `assets`) and `[deps]` (`"<path>" = "<version>"`); design 14.5.7 |
| `vactr.lock` | `get` | `# vactr.lock v1`, then one `<path> <version> sha256:<hex>` per line, sorted by path |
| `<doc>.bindings.json` | the editor, ExternalFile persistence mode only | `{"v": 1, "bindings": [...]}`; design 14.5.8 |

---

## Session Protocol (v1)

The transport is WebSocket text frames on
`ws://127.0.0.1:<port>/session?token=<hex>` (native `serve`). A handshake
with the wrong path or token gets HTTP 401. There are at most 8
connections, and a frame is at most 1 MiB. The browser passes the same
JSON text through the raw wasm ABI (see "Browser transport" below,
design 15.1.2).

Fields marked "(TASK-010)" are additive: `v` stays 1, each one is
optional, and a client that does not know it ignores it.

### Envelope

```json
{"v": 1, "seq": 12, "kind": "eval", "body": {}, "re": 11}
```

- `v`: the protocol version (1). `seq`: a per-sender counter. `re`: on
  replies only, the client `seq` being answered.
- Errors: an unknown `v` gets `protocol-error` `unsupported-version`,
  an unknown `kind` gets `unknown-kind`, a malformed body gets
  `bad-body`, and unparsable JSON gets `bad-json`. The connection stays
  open.

### Common shapes

| Name | Shape |
|------|-------|
| span | `{"start": <byte>, "end": <byte>}` in the document revision named alongside it |
| srcref | `{"file": str, "span": span, "doc_revision": int, "form_gen": int}` |
| diagnostic | `{"code": str, "severity": "error"\|"warning"\|"hint", "message": str, "span": span, "file": str, "slot"?: str, "beat"?: [num, den]}` |
| site | `{"id": int, "span": span, "tier": "direct"\|"reeval"\|"manual", "origin": "pattern-literal"\|"binding"\|"inst-default", "value": number, "form_gen": int, "key"?: str, "call"?: call}` (`key` is the BindingKey `label.site.n.param` or `label.param`, when the site is labeled) |
| call (TASK-010) | `{"name": str, "head": span, "ordinal": int (1-based among same-named calls in the top-level form), "arg": int (0-based argument index), "param"?: str}`: the nearest enclosing symbol-headed call the literal is an argument of, directly or inside a list or pattern argument. `param` is the named-argument keyword, else the declared parameter at that position. It is absent when unknown (design 15.1.2 G3) |
| editor-decl (TASK-010) | `{"name": str, "kind": "eq-curve"\|"filter-response"\|"dynamics-transfer"\|"envelope-shape"\|"delay-taps"\|"reverb-room"\|"sampler-wave"\|"wavetable-frames"\|"granular-region"\|"lfo-shape"\|"stereo-field"\|"xy-pad"\|"euclid-ring"\|"probability-dial"\|"length-handle"\|"scalar", "multiband"?: bool, "params": [{"name": str, "ctl"?: int, "range": [float, float], "curve": "linear"\|"log"\|"stepped", "unit": "none"\|"db"\|"s"\|"ms"\|"hz"\|"st", "group": int}]}`. Pattern functions carry no `ctl` |
| change | `{"from": <byte>, "to": <byte>, "insert_len": int}` (base-revision offsets) |

### Client to session

| Kind | Body |
|------|------|
| `eval` | `{"file": str, "code": str (full document text), "span"?: span, "doc_revision": int, "edit_epoch": int}` |
| `hush` | `{}` |
| `stop` | `{"slot": str}` |
| `set-var` | `{"file": str, "name": str, "value": number\|bool, "defining_form_gen": int, "edit_epoch": int}` |
| `set-tweak` | `{"file": str, "id": int, "form_gen": int, "value": number, "edit_epoch": int}` |
| `doc-changed` | `{"file": str, "doc_revision": int, "base_revision": int, "changes": [change], "dirty": [span] (new-revision), "edit_epoch": int}` |
| `learn` | `{"file": str, "binding": str (key) \| int (tweak id), "cc": int, "ch"?: int, "edit_epoch": int}` |
| `subscribe` | `{"telemetry": bool, "levels": bool, "diagnostics": bool}` |
| `manifest?` | `{}` |

### Session to client

| Kind | Routed to | Body |
|------|-----------|------|
| `eval-result` | requester | `{"file": str, "doc_revision": int, "forms": [{"span": span, "value"?: str, "failure"?: diagnostic, "form_gen": int}], "diagnostics": [diagnostic], "sites": [site], "directives": {"file_level": {...}, "entries": [...]}}` |
| `stale-binding` | requester | `{"target": int (tweak id) \| str (name), "reason": "stale-form-gen"\|"edit-invalidated"\|"unreconciled-edit"\|"superseded-definition", "current_form_gen"?: int}` |
| `directive-edit` | requester | `{"file": str, "doc_revision": int, "span": span, "expected": str, "text": str}` |
| `manifest` | requester | `{"sounds": [str], "synths": [str], "controls": [str], "editors"?: [editor-decl]}` (`editors` TASK-010: every builtin's EditorDecl plus the pattern-function table, design 15.1.2 G2) |
| `protocol-error` | requester | `{"code": "bad-json"\|"unsupported-version"\|"unknown-kind"\|"bad-body", "message": str}` |
| `bindings` | subscribers | `{"pass": int, "changed": [{"name": str, "value": str, "form_gen": int}], "sites": [site], "states": [{"name": str, "state": "ok"\|"failed"\|"blocked", "value": str, "blocked_on"?: str, "diagnostic"?: diagnostic}]}`; exactly one per completed reactive pass |
| `diag` | subscribers (`diagnostics`) | `{"add": [diagnostic], "clear": [{"slot": str}]}` |
| `playing` | subscribers (`telemetry`) | `{"events": [{"slot": str, "beat": [num, den], "time": float, "dur": [num, den], "src"?: srcref}]}` |
| `levels` | subscribers (`levels`) | `{"levels": [{"source": ":master", "rms": float, "bands"?: [float x 8]}], "analyzers"?: [{"bus": str, "kind": str, "id": int, "cells": [float]}]}` (master only in v1), at most 10 per second. TASK-010: `bands` holds the host FFT bands, and `analyzers` lists every analyzer unit of the installed bus graph with a constant `id` and its current cells (design 12.5, 15.1.2 G4) |
| `tempo` | subscribers | `{"bpm": float, "beats_per_cycle": int, "cycle": [num, den], "clock"?: {"source": "internal"\|"midi", "locked"?: bool}}` on change (a clock change counts; `clock` TASK-010) |

Ordering: `eval-result` comes before any `bindings` batch its eval
triggered. Nothing is published mid-pass.

### Browser transport (raw wasm ABI, TASK-010)

The editor initializes wasm #1 with `session_init` instead of the dev
harness's `main_init`. It uses one or the other, never both. Byte
arguments are memory from `alloc`, as in design 12.8.10. Outputs are
framed outbox records (`[u32 LE length][tag][payload]`):

| Export | Effect | Output records |
|--------|--------|----------------|
| `session_init(sample_rate f32, arena_bytes u32) -> u32` | builds the browser `Session` (browser caps, worklet cells, Directive persistence) | worklet install records |
| `session_apply(ptr, len)` | one client envelope (JSON text), connection 1 | `0x71` server envelopes (JSON text), worklet records |
| `session_tick(now f64)` | one session tick at the worklet's posted time | `0x71`, `0x72`, worklet records |
| `session_frame(now f64)` | resolves the active visual uniform plans | `0x72` |
| `session_inbox(ptr, len)`, `session_sample_put(...)` | the session twins of `inbox` and `sample_put` | — |
| `session_check(ptr, len)` | static analysis of the document text, never executes | one `0x71` record `{"kind": "check", "diagnostics": [diagnostic]}` (browser-local, not a protocol message) |
| `session_midi_in(ptr, len, time f64)` | raw MIDI bytes at an audio-clock time | — |
| `pkg_resolve(ptr, len)` | package driver step, JSON `{"proxy": url, "requirements": {path: version\|""}}` or `{"proxy": url, "lock": str}` (restore) | one `0x73` |
| `pkg_supply(url_ptr, url_len, status u32, ptr, len)` | supplies a fetched body (`200`), a not-found (`404`) or a network failure (any other status) | — |

| Tag | Payload |
|-----|---------|
| `0x71` | a server envelope as JSON text |
| `0x72` | render record JSON: `{"op": "program", "out": 0-3, "source": str, "uniform_names": [str], "assets": [{"id": int, "text": str}]}` or `{"op": "uniforms", "out": 0-3, "values": [float]}` |
| `0x73` | package driver reply JSON: `{"status": "need", "url": str}`, `{"status": "done", "lock": str, "resolved": [{"path": str, "version": str, "sha256": str}]}` or `{"status": "error", "code": str, "message": str}` |

`host.js` routes `0x70` to the console, `0x71`-`0x73` to its `onRecord`
option, and every other tag to the worklet. The worklet never sees an
editor record.

### Editor files (TASK-010)

The editor saves `.vact` as the buffer text. In ExternalFile mode it
also writes `<doc>.bindings.json`, in the format of the Files table
above. The browser stores package data in OPFS under `vactr-pkg/`:
the root requirements, the lock text, and the proxy response bodies
keyed by URL. Nothing is trusted from OPFS without validation and a
digest check (design 15.1.10). The proxy URL is kept in `localStorage`,
and the session token is never stored.
