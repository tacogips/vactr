//! TASK-008 criterion 7 (DSP half): installing an impulse response longer
//! than `max_ir_seconds` fails with `beyond-capability` whose origin span is
//! the span the install request carried. The offline-render half is
//! BE-CONTRACTS' `require(OfflineRender)` test
//! (`dsp/tests/contracts.rs`), cited rather than duplicated. Admission also
//! refuses data larger than the free arena and graph bytes over one slice.

use super::{caps, SR};
use crate::dsp::arena::{InstallRequest, ResourceKind, SampleStore, StoreKind, SLICE_BYTES};
use crate::reader::span::{FileId, Span};
use crate::types::diag::DiagCode;

fn request(kind: ResourceKind, frames: usize, origin: Span) -> InstallRequest {
    InstallRequest {
        resource: 1,
        kind,
        frames,
        channels: 1,
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        rate: SR as u32,
        bytes: 0,
        origin: Some(origin),
    }
}

#[test]
fn an_ir_over_the_tier_limit_is_beyond_capability_at_its_origin() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let origin = Span::new(FileId::new(4), 120, 160);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let two_seconds = (2.0 * SR) as usize;
    let err = store
        .admit(&request(ResourceKind::Ir, two_seconds, origin), &caps())
        .expect_err("2 s > 1 s limit");
    assert_eq!(err.code, DiagCode::BeyondCapability);
    assert_eq!(
        err.span, origin,
        "the diagnostic carries the install origin"
    );
    assert!(err.message.contains("not available on this host"));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let half = (0.5 * SR) as usize;
    assert!(store
        .admit(&request(ResourceKind::Ir, half, origin), &caps())
        .is_ok());
    // A sample of the same length is not an IR: no IR limit applies.
    assert!(store
        .admit(&request(ResourceKind::Sample, two_seconds, origin), &caps())
        .is_ok());
}

#[test]
fn arena_admission_refuses_what_does_not_fit_at_its_origin() {
    let store = SampleStore::new(StoreKind::Arena { bytes: 4096 });
    let origin = Span::new(FileId::new(2), 7, 9);
    let err = store
        .admit(&request(ResourceKind::Sample, 2048, origin), &caps())
        .expect_err("8 KB > 4 KB");
    assert_eq!(err.code, DiagCode::ArenaExhausted);
    assert_eq!(err.span, origin);
    assert!(store
        .admit(&request(ResourceKind::Sample, 1024, origin), &caps())
        .is_ok());
}

#[test]
fn oversized_graph_bytes_are_graph_too_large() {
    let store = SampleStore::new(StoreKind::Arena { bytes: 4096 });
    let origin = Span::new(FileId::new(1), 0, 3);
    let mut req = request(ResourceKind::Graph, 0, origin);
    req.bytes = SLICE_BYTES + 1;
    let err = store.admit(&req, &caps()).expect_err("over one slice");
    assert_eq!(err.code, DiagCode::GraphTooLarge);
    assert_eq!(err.span, origin);
    req.bytes = SLICE_BYTES;
    assert!(store.admit(&req, &caps()).is_ok());
}
