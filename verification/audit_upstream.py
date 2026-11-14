#!/usr/bin/env python3
"""Audit Vactr's upstream source, license and resource inventory.

The inventory in `verification/upstream_inventory.toml` lists every
Mutable Instruments or `stmlib` file that Vactr names. This script checks
it against a separate pinned checkout and against Vactr's tracked files.
It never copies upstream code or data into this repository.
"""

import argparse
import fnmatch
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parent.parent
INVENTORY = ROOT / "verification" / "upstream_inventory.toml"
NOTICES = ROOT / "THIRD_PARTY_NOTICES.md"
EURORACK_REVISION = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4"
STMLIB_REVISION = "e3bd7c9cc00e4364166f9905c0509b6ffd0535ec"

LICENSES = {"MIT", "GPL-3.0-or-later", "none"}
KINDS = {"code", "generator", "aggregate-resource", "asset"}
USES = {"source", "partial", "excluded"}
# Vactr files that may name upstream paths.
SCANNED = ("src", "tests", "examples", "verification", "editor/src",
           "design-docs/specs", "design-docs/references", "impl-plans/active",
           "THIRD_PARTY_NOTICES.md", "README.md")
# Vactr files whose numeric literals are compared with excluded tables.
LITERAL_SUFFIXES = {".rs", ".ts", ".tsx", ".js", ".py", ".cc", ".h", ".json"}
MIT_TEXT = "Permission is hereby granted, free of charge"
GPL_TEXT = "GNU General Public License"
MODULES_WITHOUT_DIR = ("dsp/", "utils/", "system/", "drivers/")
FLOAT_WINDOW = 6
BYTE_WINDOW = 12
MIN_COPY_BYTES = 256


def run(argv):
    return subprocess.run(argv, check=True, capture_output=True, text=True,
                          timeout=120).stdout


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean_tracked_tree(path):
    if run(["git", "-C", str(path), "status", "--porcelain",
            "--untracked-files=no"]).strip():
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def upstream_path(source, path):
    return source / path


def detect_license(path):
    data = path.read_bytes()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return "none"
    if MIT_TEXT in text:
        return "MIT"
    if GPL_TEXT in text:
        return "GPL-3.0-or-later"
    return "unknown"


def tracked_files():
    out = run(["git", "-C", str(ROOT), "ls-files", "-z", "--", *SCANNED])
    files = [ROOT / name for name in out.split("\0") if name]
    return [path for path in files if path.is_file()]


def load_inventory():
    data = tomllib.loads(INVENTORY.read_text())
    revisions = data.get("revisions", {})
    if revisions.get("eurorack") != EURORACK_REVISION:
        raise ValueError("inventory eurorack revision differs from the audit pin")
    if revisions.get("stmlib") != STMLIB_REVISION:
        raise ValueError("inventory stmlib revision differs from the audit pin")
    entries = {}
    for entry in data.get("file", []):
        path = entry["path"]
        if path in entries:
            raise ValueError(f"duplicate inventory entry: {path}")
        entries[path] = entry
    return entries


def check_entry_fields(entries, errors):
    for path, entry in entries.items():
        license_ = entry.get("license")
        kind = entry.get("kind")
        use = entry.get("use")
        imports = entry.get("imports", [])
        if license_ not in LICENSES:
            errors.append(f"{path}: unknown license {license_!r}")
        if kind not in KINDS:
            errors.append(f"{path}: unknown kind {kind!r}")
        if use not in USES:
            errors.append(f"{path}: unknown use {use!r}")
        if use != "excluded" and license_ != "MIT":
            errors.append(f"{path}: only MIT files may be used, not {license_}")
        if kind == "asset" and use != "excluded":
            errors.append(f"{path}: binary assets must stay excluded")
        if use == "partial" and (kind != "aggregate-resource" or not imports):
            errors.append(f"{path}: partial use needs an aggregate resource "
                          "and named imports")
        if use != "partial" and imports:
            errors.append(f"{path}: imports are allowed only for partial use")
        if use != "source" and not entry.get("reason"):
            errors.append(f"{path}: {use} use needs a reason")


def check_upstream_files(source, entries, errors):
    for path, entry in entries.items():
        file = upstream_path(source, path)
        if not file.is_file():
            errors.append(f"{path}: not present at the pinned revision")
            continue
        detected = detect_license(file)
        if detected != entry.get("license"):
            errors.append(f"{path}: declared {entry.get('license')}, "
                          f"header says {detected}")


# ---------------------------------------------------------------------------
# Upstream paths named by Vactr


PATH_TOKEN = re.compile(r"(?<![\w/.-])([a-z0-9_{},]+/[A-Za-z0-9_./{},-]+)")
NOTICE_TOKEN = re.compile(r"`([A-Za-z0-9_./{},-]+)`")
UPSTREAM_SUFFIX = re.compile(r"\.(?:cc|h|py|bin|hex)$")
BRACE = re.compile(r"\{([^{}]*)\}")


def expand(token):
    """Expand every `{a,b}` group, as in `dir/{x,y}.{h,cc}`."""
    match = BRACE.search(token)
    if not match:
        return [token]
    names = []
    for option in match.group(1).split(","):
        names.extend(expand(token[:match.start()] + option +
                            token[match.end():]))
    return names


def upstream_names(token):
    token = token.rstrip(".,")
    return [name for name in expand(token) if UPSTREAM_SUFFIX.search(name)]


def upstream_roots(source):
    roots = {p.name for p in source.iterdir() if p.is_dir()
             and not p.name.startswith(".")}
    return roots


def named_paths(files, roots):
    """Fully qualified upstream paths named anywhere in scanned Vactr files."""
    named = {}
    for file in files:
        try:
            text = file.read_text()
        except UnicodeDecodeError:
            continue
        for match in PATH_TOKEN.finditer(text):
            for path in upstream_names(match.group(1)):
                if path.split("/", 1)[0] not in roots:
                    continue
                named.setdefault(path, set()).add(
                    str(file.relative_to(ROOT)))
    return named


def notice_sections():
    text = NOTICES.read_text()
    sections = []
    for block in re.split(r"^## ", text, flags=re.M)[1:]:
        title, _, body = block.partition("\n")
        sections.append((title.strip(), body))
    return sections


def resolve_notice_names(source, all_files, roots, errors):
    """Map each notice section's relative or full names to upstream paths."""
    by_path = {}
    for title, body in notice_sections():
        module = next((word for word in re.findall(r"[a-z0-9]+", title.lower())
                       if word in roots and word != "stmlib"), None)
        for match in NOTICE_TOKEN.finditer(body):
            for name in upstream_names(match.group(1)):
                if name.startswith("verification/") or "/" not in name and \
                        not module:
                    continue
                resolved = resolve_name(name, module, all_files)
                if resolved is None:
                    errors.append(f"notice section {title!r}: {name} does not "
                                  "resolve to one pinned upstream file")
                    continue
                by_path.setdefault(resolved, set()).add(title)
    return by_path


def resolve_name(name, module, all_files):
    if name in all_files:
        return name
    if name.startswith(MODULES_WITHOUT_DIR) and f"stmlib/{name}" in all_files:
        return f"stmlib/{name}"
    if not module:
        return None
    prefixes = [f"{module}/"]
    if module == "tides":
        prefixes.append("tides2/")
    for prefix in prefixes:
        found = [path for path in all_files
                 if path.startswith(prefix) and path.endswith("/" + name)]
        if len(found) == 1:
            return found[0]
    return None


def notice_carrying_sources(files, entries):
    """Upstream paths named by a Vactr source file that retains an MIT notice."""
    named = set()
    for file in files:
        if file.suffix != ".rs":
            continue
        text = file.read_text()
        if MIT_TEXT not in text:
            continue
        for path in entries:
            if path in text:
                named.add(path)
    return named


# ---------------------------------------------------------------------------
# Transitive include closure


INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"', re.M)


def include_closure(source, starts):
    """Map each transitively included file to the file that first includes it."""
    parent = {path: None for path in starts if path.endswith((".cc", ".h"))}
    pending = list(parent)
    while pending:
        path = pending.pop(0)
        file = source / path
        if not file.is_file():
            continue
        for included in INCLUDE.findall(file.read_text(errors="replace")):
            if included not in parent and (source / included).is_file():
                parent[included] = path
                pending.append(included)
    return parent


def include_chain(parent, path):
    chain = [path]
    while parent.get(chain[-1]) is not None:
        chain.append(parent[chain[-1]])
    return " <- ".join(chain)


# ---------------------------------------------------------------------------
# Numeric-window comparison


NUMBER = re.compile(
    r"(?<![\w.])-?(?:0[xX][0-9a-fA-F_]+|\d[\d_]*(?:\.\d[\d_]*)?"
    r"(?:[eE][+-]?\d+)?)(?:_?(?:f32|f64|[iu](?:8|16|32|64|size)))?(?![\w.])")
LINE_COMMENT = re.compile(r"//[^\n]*")
SEPARATOR = re.compile(r"^[\s,]*$")


def parse_number(token):
    token = re.sub(r"_?(?:f32|f64|[iu](?:8|16|32|64|size))$", "", token)
    token = token.replace("_", "")
    negative = token.startswith("-")
    body = token[1:] if negative else token
    value = int(body, 16) if body.lower().startswith("0x") else float(body)
    return -value if negative else value


def numeric_runs(text):
    runs = []
    current = []
    last_end = None
    for match in NUMBER.finditer(text):
        if last_end is not None:
            gap = LINE_COMMENT.sub("", text[last_end:match.start()])
            if not SEPARATOR.match(gap):
                if current:
                    runs.append(current)
                current = []
        try:
            current.append(parse_number(match.group(0)))
        except ValueError:
            current = []
        last_end = match.end()
    if current:
        runs.append(current)
    return runs


def key(value):
    return f"{float(value):.5g}"


def trivial(window):
    if len(set(window)) < 4:
        return True
    steps = {round(float(b) - float(a), 9) for a, b in zip(window, window[1:])}
    return len(steps) == 1


def windows(values, size):
    for start in range(len(values) - size + 1):
        window = values[start:start + size]
        if not trivial(window):
            yield tuple(key(value) for value in window)


def vactr_windows(files):
    index = {}
    for file in files:
        if file.suffix not in LITERAL_SUFFIXES:
            continue
        try:
            text = file.read_text()
        except UnicodeDecodeError:
            continue
        for run_ in numeric_runs(text):
            for size in (FLOAT_WINDOW, BYTE_WINDOW):
                for window in windows(run_, size):
                    index.setdefault(window, str(file.relative_to(ROOT)))
    return index


TABLE = re.compile(
    r"^(?:const\s+)?[A-Za-z0-9_]+\s+([A-Za-z0-9_]+)\[[^\]]*\]\s*=\s*\{"
    r"(.*?)^\};", re.M | re.S)


def resource_tables(file):
    text = file.read_text(errors="replace")
    tables = {}
    for match in TABLE.finditer(text):
        values = []
        for run_ in numeric_runs(LINE_COMMENT.sub("", match.group(2))):
            values.extend(run_)
        if values:
            tables[match.group(1)] = values
    return tables


def asset_tables(file):
    data = file.read_bytes()
    words = [int.from_bytes(data[i:i + 2], "little", signed=True)
             for i in range(0, len(data) - 1, 2)]
    return {
        "u8": list(data),
        "i16": words,
        "i16/32768": [word / 32768 for word in words],
    }


def scaled(values):
    if all(isinstance(value, int) or float(value).is_integer()
           for value in values[:64]):
        return {"raw": values, "/32768": [v / 32768 for v in values]}
    return {"raw": values}


def check_tables(source, entries, index, errors, report):
    found = []
    for path, entry in entries.items():
        if entry["kind"] not in ("aggregate-resource", "asset") or \
                entry["use"] == "source":
            continue
        file = source / path
        if not file.is_file():
            continue
        if entry["kind"] == "asset":
            tables = {name: {"raw": values}
                      for name, values in asset_tables(file).items()}
        else:
            tables = {name: scaled(values)
                      for name, values in resource_tables(file).items()}
        allowed = entry.get("imports", [])
        used_patterns = set()
        for name, variants in tables.items():
            for variant, values in variants.items():
                size = BYTE_WINDOW if all(
                    float(value).is_integer() for value in values) \
                    else FLOAT_WINDOW
                hit = next((index[w] for w in windows(values, size)
                            if w in index), None)
                if hit is None:
                    continue
                pattern = next((p for p in allowed
                                if fnmatch.fnmatchcase(name, p)), None)
                found.append({"upstream": path, "table": name,
                              "encoding": variant, "vactr": hit,
                              "allowed": pattern is not None})
                if pattern is None:
                    errors.append(f"{path}: table {name} ({variant}) appears "
                                  f"in {hit} but is not a declared import")
                else:
                    used_patterns.add(pattern)
                break
        for pattern in allowed:
            if pattern not in used_patterns:
                report["warnings"].append(
                    f"{path}: declared import {pattern} was not found in Vactr")
    report["table_matches"] = found


def check_copies(source, files, errors):
    upstream = {}
    for file in source.rglob("*"):
        if ".git" in file.parts or not file.is_file():
            continue
        data = file.read_bytes()
        if len(data) >= MIN_COPY_BYTES:
            upstream[hashlib.sha256(data).hexdigest()] = \
                str(file.relative_to(source))
    for file in files:
        digest = hashlib.sha256(file.read_bytes()).hexdigest()
        if digest in upstream:
            errors.append(f"{file.relative_to(ROOT)} is a byte-identical copy "
                          f"of upstream {upstream[digest]}")


# ---------------------------------------------------------------------------


def audit(source):
    errors = []
    report = {"eurorack_revision": EURORACK_REVISION,
              "stmlib_revision": STMLIB_REVISION, "warnings": []}
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"reference checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")

    entries = load_inventory()
    check_entry_fields(entries, errors)
    check_upstream_files(source, entries, errors)

    all_files = {str(p.relative_to(source)) for p in source.rglob("*")
                 if p.is_file() and ".git" not in p.parts}
    roots = upstream_roots(source)
    files = tracked_files()

    named = named_paths(files, roots)
    by_notice = resolve_notice_names(source, all_files, roots, errors)
    for path, where in sorted(named.items()):
        if path not in all_files:
            errors.append(f"{path} (named in {', '.join(sorted(where))}) is "
                          "not a pinned upstream file")
        elif path not in entries:
            errors.append(f"{path} (named in {', '.join(sorted(where))}) is "
                          "missing from the inventory")
    for path in sorted(by_notice):
        if path not in entries:
            errors.append(f"{path} (notice {', '.join(sorted(by_notice[path]))}) "
                          "is missing from the inventory")

    carried = notice_carrying_sources(files, entries)
    for path, entry in entries.items():
        if entry["use"] != "excluded" and path not in by_notice and \
                path not in carried:
            errors.append(f"{path}: used file is not named by any notice")

    used = [path for path, entry in entries.items()
            if entry["use"] != "excluded" and entry["kind"] == "code"]
    closure = include_closure(source, used)
    transitive = sorted(set(closure) - set(entries))
    report["non_mit_includes"] = {}
    for path in sorted(closure):
        detected = detect_license(source / path)
        if detected == "MIT":
            continue
        chain = include_chain(closure, path)
        report["non_mit_includes"][path] = chain
        if entries.get(path, {}).get("use") != "excluded":
            errors.append(f"{path}: transitive {detected} include must be "
                          f"inventoried as excluded ({chain})")
    report["transitive_includes"] = transitive
    report["stmlib_closure"] = sorted(p for p in closure
                                      if p.startswith("stmlib/"))

    check_tables(source, entries, vactr_windows(files), errors, report)
    check_copies(source, files, errors)

    counts = {}
    for entry in entries.values():
        counts[entry["use"]] = counts.get(entry["use"], 0) + 1
    report["inventory"] = {"files": len(entries), **counts}
    report["errors"] = errors
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path,
                        help="separate Eurorack checkout with stmlib submodule")
    parser.add_argument("--report", type=Path,
                        help="write the full JSON report to this path")
    args = parser.parse_args()
    report = audit(args.source.resolve())
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    summary = {key: report[key] for key in
               ("eurorack_revision", "stmlib_revision", "inventory")}
    summary["transitive_includes"] = len(report["transitive_includes"])
    summary["stmlib_closure"] = report["stmlib_closure"]
    summary["non_mit_includes"] = report["non_mit_includes"]
    summary["table_matches"] = report["table_matches"]
    summary["warnings"] = report["warnings"]
    summary["errors"] = report["errors"]
    print(json.dumps(summary, indent=2))
    return 1 if report["errors"] else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError,
            tomllib.TOMLDecodeError) as error:
        print(f"audit_upstream: {error}", file=sys.stderr)
        sys.exit(2)
