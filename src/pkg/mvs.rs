//! Minimal version selection (design 5.7, 14.5.7), as Go: a breadth-first
//! walk of the requirement graph over each visited version's manifest,
//! then the MAXIMUM of the MINIMUM requirements per path.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::ns::pkg::PackageId;
use crate::pkg::semver::Version;
use crate::pkg::store::{PackageStore, PkgError};

/// The selected version of every package reachable from `roots`, sorted
/// bytewise by path.
///
/// # Errors
/// `Unresolvable` when a required version is not in the store; the store's
/// errors otherwise.
pub fn mvs_resolve(
    roots: &[(PackageId, Version)],
    store: &mut dyn PackageStore,
) -> Result<Vec<(PackageId, Version)>, PkgError> {
    let mut selected: BTreeMap<PackageId, Version> = BTreeMap::new();
    let mut listed: BTreeMap<PackageId, Vec<Version>> = BTreeMap::new();
    let mut visited: BTreeSet<(PackageId, Version)> = BTreeSet::new();
    let mut queue: VecDeque<(PackageId, Version)> = roots.iter().cloned().collect();
    while let Some((id, version)) = queue.pop_front() {
        if !visited.insert((id.clone(), version.clone())) {
            continue;
        }
        if !listed.contains_key(&id) {
            let versions = store.list_versions(&id)?;
            listed.insert(id.clone(), versions);
        }
        if !listed[&id].contains(&version) {
            return Err(PkgError::Unresolvable(format!(
                "cannot resolve `{}`: no version {version}",
                id.0
            )));
        }
        let manifest = store.manifest(&id, &version)?;
        for (dep, min) in manifest.deps {
            if !visited.contains(&(dep.clone(), min.clone())) {
                queue.push_back((dep, min));
            }
        }
        match selected.get_mut(&id) {
            Some(v) if *v >= version => {}
            Some(v) => *v = version,
            None => {
                selected.insert(id, version);
            }
        }
    }
    let mut out: Vec<(PackageId, Version)> = selected.into_iter().collect();
    out.sort_by(|a, b| a.0 .0.as_bytes().cmp(b.0 .0.as_bytes()));
    Ok(out)
}
