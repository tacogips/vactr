//! Complete finite output through the isolated original candidate and native exporter.
use std::path::Path;

/// Render a complete finite Song; no manual cycle cap or active live evaluation.
pub fn main(source: &Path, output: &Path, sample_rate: u32, cwd: &Path) -> i32 {
    #[cfg(feature = "host-native")]
    {
        use crate::host::native::NativeSampleLoader;
        use crate::session::song::{evaluate_song_candidate, CandidateBuildCtx};
        use crate::song::export::{export_song, SongExportOptions};
        use crate::song::{prepare_song, SnapshotEpoch};
        let text = match std::fs::read_to_string(source) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("vactr: cannot read `{}`: {error}", source.display());
                return 1;
            }
        };
        let loader = NativeSampleLoader::new(&[cwd.to_owned()], cwd);
        let factory = loader.isolated_song_factory();
        let lock = super::read_lock(cwd);
        let cache = super::open_cache();
        let cx = CandidateBuildCtx {
            assets: factory.as_ref(),
            asset_limits: asset_limits(),
            lock: lock.as_ref(),
            cache: cache.as_deref(),
        };
        let file = source.to_string_lossy();
        let prepared = match evaluate_song_candidate(&text, &file, 1, SnapshotEpoch(1), &cx)
            .and_then(prepare_song)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                eprintln!("vactr: {}: {}", error.code.as_str(), error.message);
                return 3;
            }
        };
        match export_song(
            prepared,
            &SongExportOptions {
                output: output.to_owned(),
                sample_rate,
            },
        ) {
            Ok(report) => {
                for warning in &report.warnings {
                    eprintln!("vactr: warning: {warning}");
                }
                println!(
                    "rendered {} frames at {} Hz; revision {}, epoch {}, seed {}; state {:?}",
                    report.total_frames,
                    report.sample_rate,
                    report.revision,
                    report.epoch.0,
                    report.seed,
                    report.final_state
                );
                0
            }
            Err(error) => {
                eprintln!("vactr: {}: {}", error.code.as_str(), error.message);
                3
            }
        }
    }
    #[cfg(not(feature = "host-native"))]
    {
        let _ = (source, output, sample_rate, cwd);
        eprintln!("vactr: finite render requires the host-native feature");
        1
    }
}

#[cfg(feature = "host-native")]
pub(super) fn asset_limits() -> crate::song::assets::SongAssetLimits {
    crate::song::assets::SongAssetLimits {
        max_resources: 256,
        max_pcm_bytes: 128 * 1024 * 1024,
        max_source_files: 64,
        max_source_bytes: 1_000_000,
        max_banks: 64,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    }
}
#[cfg(feature = "host-native")]
pub(super) fn preparation_limits() -> crate::host::caps::SongPreparationLimits {
    crate::host::caps::SongPreparationLimits {
        capabilities: crate::dsp::caps::CapabilitySet::native(),
        song: crate::song::SongLimits::default(),
        max_resources: 256,
        max_pending_records: 4096,
        max_graph_bytes: 1_000_000,
        max_work: 8_000_000,
    }
}
