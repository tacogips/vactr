# Command Design

This document specifies the `vactrol` command-line interface and the
editor-runtime session protocol on the wire. The behavior rules behind
them (the eval pipeline, reactive publication, write authority,
packages, directives, the socket's security rules) are in
`design-implementation.md` sections 5.7, 13.5, 14 and 14.5.

## Overview

`vactrol` is a single binary. The `lsp` verb needs the `lsp` cargo
feature; every other verb is in the default build (`host-native`).
Every verb runs one `Session` on the main thread (design 14.5.4).

---

## Sections

### Naming (decided 2026-09-24)

| Item | Value |
|------|-------|
| Language | Vactrol (renamed from nagamu; a vactrol is an LED coupled to a photoresistor: light controlling sound) |
| Binary and crate | `vactrol` |
| Source file extension | `.vact` |
| Repository | https://github.com/tacogips/vactrol |

### Subcommands

| Verb | Synopsis | Behavior |
|------|----------|----------|
| `repl` | `vactrol repl [--host H]` | Interactive console over one session (design 14.2, 14.5.10). Sound keeps playing between lines. Completed expressions bind `_1`, `_2`, …; a failed one binds nothing. EOF (Ctrl-D) exits. |
| `run` | `vactrol run <file.vact> [--host H] [--cycles N]` | Evaluates the whole document once (packages from `vactrol.lock` + cache). Prints diagnostics and console output, then keeps ticking: until interrupted, or for N cycles when `--cycles` is given. |
| `serve` | `vactrol serve [<file.vact>] [--host H] [--port P] [--bind 127.0.0.1]` | Starts the session socket, optionally evaluating a file first, and prints the connect URL with the token once to stderr. Serves until interrupted. |
| `get` | `vactrol get [<package-path>[@<version>]] [--store dir:<root>]` | With a path: adds or raises the requirement in `./vactrol.toml` (created with only `[deps]` if absent). Then, and also with no path: runs minimal version selection, fetches and verifies every selected package into the cache, and writes `./vactrol.lock`. The default store is git; `--store dir:<root>` uses a local-directory store (`<root>/<path>@<version>/`). |
| `lsp` | `vactrol lsp [--session <ws-url>]` | Language server over stdio (design 14.3, 14.5.11). With `--session` it attaches to a running `serve` socket for runtime diagnostics. A build without the `lsp` feature prints an error and exits 1. |
| (none) | `vactrol --version`, `vactrol help` | Prints the version or the usage. |

A package path is `github.com/<owner>/<name>` (lowercase), and a
version is a git tag `vMAJOR.MINOR.PATCH[-pre]`. With no version given,
`get` picks the highest non-prerelease tag.

### Flags and Options

| Flag | Verbs | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--host` | repl, run, serve | `native` \| `noop` | `native` | Audio/MIDI host. If the native device fails to open, the CLI warns on stderr and falls back to `noop`. |
| `--cycles` | run | positive int | (run until interrupted) | Stop after N cycles. With `--host noop` a virtual clock is used, so the run ends immediately and deterministically. |
| `--port` | serve | u16 | `0` (OS-assigned) | TCP port. The chosen port is printed in the connect URL. |
| `--bind` | serve | IP address | `127.0.0.1` | Only loopback addresses are accepted; anything else is a usage error. |
| `--store` | get | `dir:<path>` | (git) | Resolve and fetch from a local-directory store instead of git. |
| `--session` | lsp | ws URL with token | (none) | Attach to a live session socket. |

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `VACTROL_HOME` | No | `~/.vactrol` | Root for the package cache `$VACTROL_HOME/pkg/<path>@<version>` and its staging area `$VACTROL_HOME/pkg/.staging/`. |
| `HOME` | Yes when `VACTROL_HOME` is unset | (OS) | Used to derive the default `VACTROL_HOME`. |

The git store runs `git` with `GIT_TERMINAL_PROMPT=0` and
`GIT_CONFIG_NOSYSTEM=1` set for that child process only. No verb reads
or stores secrets. The session token exists only in memory and in the
printed URL.

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success. For `run`, the document evaluated with no error-severity diagnostic and no failed form. |
| 1 | General error: IO, audio host, package resolution or integrity (`get`), socket bind. |
| 2 | Usage error: unknown verb or flag, missing argument, non-loopback `--bind`. |
| 3 | `run` only: the document reported error-severity diagnostics or failed forms. The run still played for the requested cycles and printed everything. |

### Files

| File | Written by | Format |
|------|------------|--------|
| `vactrol.toml` | the user; `get` adds `[deps]` entries | TOML subset: `[package]` (`path`, `vactrol`, `assets`) and `[deps]` (`"<path>" = "<version>"`); design 14.5.7 |
| `vactrol.lock` | `get` | `# vactrol.lock v1`, then one `<path> <version> sha256:<hex>` per line, sorted by path |
| `<doc>.bindings.json` | the editor, ExternalFile persistence mode only | `{"v": 1, "bindings": [...]}`; design 14.5.8 |

---

## Session Protocol (v1)

The transport is WebSocket text frames on
`ws://127.0.0.1:<port>/session?token=<hex>` (native `serve`). A handshake
with the wrong path or token gets HTTP 401. There are at most 8
connections, and a frame is at most 1 MiB. The browser uses the same
JSON shapes over direct calls (TASK-010).

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
| site | `{"id": int, "span": span, "tier": "direct"\|"reeval"\|"manual", "origin": "pattern-literal"\|"binding"\|"inst-default", "value": number, "form_gen": int, "key"?: str}` (`key` is the BindingKey `label.site.n.param` or `label.param`, when the site is labeled) |
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
| `manifest` | requester | `{"sounds": [str], "synths": [str], "controls": [str]}` |
| `protocol-error` | requester | `{"code": "bad-json"\|"unsupported-version"\|"unknown-kind"\|"bad-body", "message": str}` |
| `bindings` | subscribers | `{"pass": int, "changed": [{"name": str, "value": str, "form_gen": int}], "sites": [site], "states": [{"name": str, "state": "ok"\|"failed"\|"blocked", "value": str, "blocked_on"?: str, "diagnostic"?: diagnostic}]}`; exactly one per completed reactive pass |
| `diag` | subscribers (`diagnostics`) | `{"add": [diagnostic], "clear": [{"slot": str}]}` |
| `playing` | subscribers (`telemetry`) | `{"events": [{"slot": str, "beat": [num, den], "time": float, "dur": [num, den], "src"?: srcref}]}` |
| `levels` | subscribers (`levels`) | `{"levels": [{"source": ":master", "rms": float}]}` (master only in v1), at most 10 per second |
| `tempo` | subscribers | `{"bpm": float, "beats_per_cycle": int, "cycle": [num, den]}` on change |

Ordering: `eval-result` comes before any `bindings` batch its eval
triggered. Nothing is published mid-pass.
