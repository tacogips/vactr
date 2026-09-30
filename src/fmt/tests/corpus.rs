//! Fixed-point and guarantee checks over every committed Vact source.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::check_guarantees;
use crate::fmt::{format, Outcome};

pub(super) fn committed_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tracked = Command::new("git")
        .args(["ls-files", "--", "*.vact"])
        .current_dir(root)
        .output();
    let paths = match tracked {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(PathBuf::from)
            .collect(),
        _ => fallback_paths(root),
    };
    paths
        .into_iter()
        .filter_map(|path| {
            let contents = fs::read_to_string(root.join(&path)).ok()?;
            Some((path.to_string_lossy().into_owned(), contents))
        })
        .collect()
}

fn fallback_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_vact_files(&root.join("examples"), &mut paths);
    collect_vact_files(&root.join("src/prelude"), &mut paths);
    paths
}

fn collect_vact_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_vact_files(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "vact")
        {
            if let Ok(relative) = path.strip_prefix(env!("CARGO_MANIFEST_DIR")) {
                paths.push(relative.to_path_buf());
            }
        }
    }
}

#[test]
fn every_committed_vact_file_is_a_fixed_point_with_guarantees() {
    let sources = committed_sources();
    assert!(
        sources.len() >= 10,
        "expected at least 10 committed .vact files"
    );
    for (path, src) in sources {
        let formatted = format(&src);
        assert_eq!(
            formatted.text, src,
            "committed source is not a fixed point: {path}"
        );
        assert_eq!(formatted.outcome, Outcome::Unchanged, "{path}");
        check_guarantees(&src);
    }
}
