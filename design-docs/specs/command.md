# Command Design

This document describes CLI command interface design specifications.

## Overview

Command-line interface design decisions, including subcommands, flags, options, and environment variables.

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

Define the CLI subcommand structure and hierarchy.

### Flags and Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| (Add flags here) | | | |

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| (Add env vars here) | | | |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| (Add more exit codes as needed) | |

---
