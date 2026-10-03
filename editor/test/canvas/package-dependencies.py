"""Read-only preservation checks for CE-PACKAGE; never regenerate locks."""
from pathlib import Path
import argparse
import hashlib
import json
import sys
import subprocess
# macOS system Python may be 3.9; use the repository's installed mise Python.
try:
    import tomllib
except ModuleNotFoundError:
    import os
    import subprocess
    resolved = subprocess.check_output(
        ['mise', 'which', 'python3'], cwd=Path(__file__).resolve().parents[3], text=True
    ).strip()
    if Path(resolved).resolve() == Path(sys.executable).resolve():
        raise RuntimeError('Python 3.11+ with tomllib is required')
    os.execv(resolved, [resolved, str(Path(__file__).resolve()), *sys.argv[1:]])

ROOT = Path(__file__).resolve().parents[3]
IDENTITY = lambda p: (p['name'], p['version'], p.get('source'), p.get('checksum'))
PATHS = ['Cargo.toml', 'Cargo.lock', 'editor/package.json', 'editor/package-lock.json',
         'editor/src-tauri/Cargo.toml', 'editor/src-tauri/Cargo.lock',
         'editor/test/canvas/package-browser-smoke.mjs', 'editor/test/canvas/package-dependencies.py']

def hashes():
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in PATHS}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument('--baseline', type=Path)
    inputs.add_argument('--current-intake', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = dict(testsRun=0, testsPassed=0, failureCount=0, assertions=[])
    before_hashes = hashes()

    def check(name, condition, details=None):
        result['testsRun'] += 1
        result['testsPassed'] += int(bool(condition))
        result['failureCount'] += int(not condition)
        result['assertions'].append(dict(name=name, passed=bool(condition), details=details))

    try:
        if args.current_intake:
            intake = json.loads(args.current_intake.read_text())
            records = intake['paths']
            check('intake names all expected source inputs', set(records) == set(PATHS))
            commit = intake['commit']
            resolved = subprocess.check_output(
                ['git', 'rev-parse', '--verify', commit + '^{commit}'], cwd=ROOT, text=True
            ).strip()
            check('intake commit is exact and retained in current history',
                  resolved == commit and subprocess.run(
                      ['git', 'merge-base', '--is-ancestor', commit, 'HEAD'], cwd=ROOT
                  ).returncode == 0)
            snapshots = {}
            errors = []
            for name in PATHS:
                record = records[name]
                snapshot_path = ROOT / record['snapshot']
                snapshot = snapshot_path.resolve()
                expected = (ROOT / 'tmp/canvas-editor-224/plan-amendment-237/preimages').resolve()
                if snapshot.parent != expected or snapshot_path.is_symlink():
                    raise ValueError('unexpected intake snapshot path: ' + name)
                raw = snapshot.read_bytes()
                digest = hashlib.sha256(raw).hexdigest()
                committed = subprocess.check_output(['git', 'show', commit + ':' + name], cwd=ROOT)
                if not (digest == record['currentSha256'] == record['committedSha256']
                        == hashlib.sha256(committed).hexdigest()):
                    errors.append(name)
                snapshots[name] = raw
            check('immutable snapshots match recorded hashes and admitted commit objects', not errors, errors)
            # The verifier is the sole source authorized to change in this recovery.
            drift = [name for name in PATHS if name != 'editor/test/canvas/package-dependencies.py'
                     and (ROOT / name).read_bytes() != snapshots[name]]
            check('all dependency inputs and browser runner preserved since intake', not drift, drift)
            old_cargo_bytes = snapshots['editor/src-tauri/Cargo.lock']
            old_npm_bytes = snapshots['editor/package-lock.json']
            old_cargo = tomllib.loads(old_cargo_bytes.decode())['package']
            cargo = tomllib.loads((ROOT / 'editor/src-tauri/Cargo.lock').read_text())['package']
            old_ids, ids = set(map(IDENTITY, old_cargo)), set(map(IDENTITY, cargo))
            added, removed = ids - old_ids, old_ids - ids
            check('complete Cargo lock records and identities preserved', cargo == old_cargo and not added and not removed)
            old_npm = json.loads(old_npm_bytes)['packages']
            npm = json.loads((ROOT / 'editor/package-lock.json').read_text())['packages']
            check('complete npm lock records including root preserved', npm == old_npm)
            result.update(mode='current-intake', intakeCommit=commit, intakePath=str(args.current_intake),
                          preservedCargoIdentities=len(old_ids & ids),
                          addedCargoIdentities=sorted(added, key=str), removedCargoIdentities=sorted(removed, key=str))
        else:
            baseline = json.loads((args.baseline / 'baseline.json').read_text())
            old_cargo_bytes = (args.baseline / 'edit-002/1').read_bytes()
            old_npm_bytes = (args.baseline / 'edit-001/1').read_bytes()
            check('baseline snapshots match recorded pre-preparation hashes',
                  hashlib.sha256(old_cargo_bytes).hexdigest() == baseline['editor/src-tauri/Cargo.lock']
                  and hashlib.sha256(old_npm_bytes).hexdigest() == baseline['editor/package-lock.json'])
            old_cargo = tomllib.loads(old_cargo_bytes.decode())['package']
            cargo = tomllib.loads((ROOT / 'editor/src-tauri/Cargo.lock').read_text())['package']
            old_ids, ids = set(map(IDENTITY, old_cargo)), set(map(IDENTITY, cargo))
            added, removed = ids - old_ids, old_ids - ids
            result.update(preservedCargoIdentities=len(old_ids & ids),
                          addedCargoIdentities=sorted(added, key=str), removedCargoIdentities=sorted(removed, key=str))
            check('all 430 original Cargo identities and checksums preserved', len(old_ids) == 430 and not removed)
            old_npm = json.loads(old_npm_bytes)['packages']
            npm = json.loads((ROOT / 'editor/package-lock.json').read_text())['packages']
            changed = [p for p, v in old_npm.items() if p and npm.get(p) != v]
            check('existing npm package records unchanged', not changed, changed)
        package = json.loads((ROOT / 'editor/package.json').read_text())
        check('exact Playwright manifest and lock pins',
              package['devDependencies']['playwright'] == '1.62.1'
              and npm['']['devDependencies']['playwright'] == '1.62.1'
              and all(npm['node_modules/' + n]['version'] == '1.62.1' for n in ['playwright', 'playwright-core']))
        additions = {p: v for p, v in npm.items() if p not in old_npm}
        allowed = {'node_modules/playwright', 'node_modules/playwright-core', 'node_modules/playwright/node_modules/fsevents'}
        check('npm additions restricted to pinned automation closure',
              set(additions) <= allowed and all(v.get('resolved', '').startswith('https://registry.npmjs.org/')
              and v.get('integrity', '').startswith('sha512-') for v in additions.values()), additions)
        if args.current_intake:
            check('pinned automation registry integrity records', all(
                npm['node_modules/' + name].get('resolved', '').startswith('https://registry.npmjs.org/')
                and npm['node_modules/' + name].get('integrity', '').startswith('sha512-')
                for name in ['playwright', 'playwright-core']))
        manifest = tomllib.loads((ROOT / 'editor/src-tauri/Cargo.toml').read_text())
        check('local root native dependency contract', manifest['dependencies']['vactr'] ==
              dict(path='../..', **{'default-features': False}, features=['host-native']))
        check('Cargo registry checksums and sources', all(
              not p.get('source') or p['source'] == 'registry+https://github.com/rust-lang/crates.io-index'
              and bool(p.get('checksum')) for p in cargo))
        # Lock dependency strings disambiguate duplicate names using version/source.
        reached = set()
        pending = [p for p in cargo if p['name'] == 'vactr' and not p.get('source')]
        errors = []
        check('one local vactr lock entry', len(pending) == 1)
        while pending:
            node = pending.pop()
            ident = IDENTITY(node)
            if ident in reached:
                continue
            reached.add(ident)
            for dependency in node.get('dependencies', []):
                parts = dependency.split(' ', 2)
                matches = [p for p in cargo if p['name'] == parts[0]
                           and (len(parts) < 2 or p['version'] == parts[1])
                           and (len(parts) < 3 or p.get('source') == parts[2].strip('()'))]
                if len(matches) != 1:
                    errors.append(dependency)
                else:
                    pending.extend(matches)
        check('new Cargo identities attributable to vactr closure', not errors and added <= reached,
              dict(ambiguousDependencies=errors, outsideClosure=sorted(added - reached, key=str)))
        if args.baseline:
            result['rootDriftSincePreparation'] = {p: before_hashes[p] != baseline[p] for p in ['Cargo.toml', 'Cargo.lock']}
    except Exception as error:
        check('required evidence parses successfully', False, str(error))
    result['sourceHashesBefore'] = before_hashes
    result['sourceHashesAfter'] = hashes()
    check('all checked manifests locks and runners unchanged during verification',
          result['sourceHashesBefore'] == result['sourceHashesAfter'])
    # Exclusive output creation preserves previous attempts and baseline evidence.
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as output:
        json.dump(result, output, indent=2)
        output.write('\n')
    print(json.dumps(result))
    return int(result['failureCount'] != 0)

if __name__ == '__main__':
    sys.exit(main())
