//! Checked uniform source configuration density, separate from route identity.
mod index;
use super::components::charge_members;
use super::*;
use crate::vm::fail::{FailCode, Failure};

/// Starts of configuration components, independently of note density.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct UniformBirthBound {
    slope: Ratio64,
    burst: u64,
}
impl Default for UniformBirthBound {
    fn default() -> Self {
        Self {
            slope: Ratio64::ZERO,
            burst: 0,
        }
    }
}
impl UniformBirthBound {
    fn at(self, length: Ratio64) -> Result<u64, Failure> {
        if length < Ratio64::ZERO {
            return Err(Failure::new(
                FailCode::Type,
                "negative configuration window",
            ));
        }
        if length == Ratio64::ZERO {
            return Ok(0);
        }
        scalar_ceiling(
            i128::from(self.slope.num())
                .checked_mul(i128::from(length.num()))
                .ok_or_else(capacity_overflow)?,
            i128::from(self.slope.den())
                .checked_mul(i128::from(length.den()))
                .ok_or_else(capacity_overflow)?,
        )?
        .checked_add(self.burst)
        .ok_or_else(capacity_overflow)
    }
    fn add_finite(self, other: Self, finite_total: u64) -> Result<Self, Failure> {
        let burst = self
            .burst
            .checked_add(other.burst)
            .ok_or_else(capacity_overflow)?;
        match self.slope.checked_add(other.slope) {
            Ok(slope) => Ok(Self { slope, burst }),
            Err(failure) if failure.code == FailCode::Overflow => Ok(Self {
                slope: Ratio64::ZERO,
                burst: finite_total,
            }),
            Err(failure) => Err(failure),
        }
    }
    fn scale(self, count: u64) -> Result<Self, Failure> {
        Ok(Self {
            slope: self.slope.checked_mul(integer(count)?)?,
            burst: self
                .burst
                .checked_mul(count)
                .ok_or_else(capacity_overflow)?,
        })
    }
}
/// Applicable components at a point, their start bound, and finite total.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ConfigurationBound {
    instantaneous: u64,
    births: UniformBirthBound,
    finite_total: u64,
}
impl ConfigurationBound {
    fn empty() -> Self {
        Self::default()
    }
    fn constant(count: u64) -> Self {
        Self {
            instantaneous: count,
            births: UniformBirthBound {
                slope: Ratio64::ZERO,
                burst: count,
            },
            finite_total: count,
        }
    }
    fn add(self, other: Self) -> Result<Self, Failure> {
        let finite_total = self
            .finite_total
            .checked_add(other.finite_total)
            .ok_or_else(capacity_overflow)?;
        Ok(Self {
            instantaneous: self
                .instantaneous
                .checked_add(other.instantaneous)
                .ok_or_else(capacity_overflow)?,
            births: self.births.add_finite(other.births, finite_total)?,
            finite_total,
        })
    }
    fn scale(self, count: u64) -> Result<Self, Failure> {
        Ok(Self {
            instantaneous: self
                .instantaneous
                .checked_mul(count)
                .ok_or_else(capacity_overflow)?,
            births: self.births.scale(count)?,
            finite_total: self
                .finite_total
                .checked_mul(count)
                .ok_or_else(capacity_overflow)?,
        })
    }
    fn fragmented(
        self,
        owner: crate::pattern::TimeSpan,
        source_width: Ratio64,
    ) -> Result<Self, Failure> {
        let per_cycle = self.components_intersecting(source_width)?;
        let births = UniformBirthBound {
            slope: integer(per_cycle)?,
            burst: per_cycle.checked_mul(2).ok_or_else(capacity_overflow)?,
        };
        let finite_total = self
            .instantaneous
            .checked_add(births.at(owner.end.checked_sub(owner.begin)?)?)
            .ok_or_else(capacity_overflow)?;
        Ok(Self {
            instantaneous: self.instantaneous,
            births,
            finite_total,
        })
    }
    pub(super) fn components_intersecting(self, length: Ratio64) -> Result<u64, Failure> {
        Ok(self
            .instantaneous
            .checked_add(self.births.at(length)?)
            .ok_or_else(capacity_overflow)?
            .min(self.finite_total))
    }
    pub(super) fn live_generations(self, tail_cycles: Ratio64) -> Result<u64, Failure> {
        self.components_intersecting(tail_cycles)
    }
}
pub(super) fn density_charge(remaining: &mut u32, work: u32) -> Result<(), Failure> {
    *remaining = remaining.checked_sub(work).ok_or_else(|| {
        Failure::new(
            FailCode::FuelExhausted,
            "configuration density work exhausted",
        )
    })?;
    Ok(())
}
fn density_owner_width(owner: crate::pattern::TimeSpan) -> Result<Ratio64, Failure> {
    if owner.end < owner.begin {
        return Err(Failure::new(FailCode::Type, "reversed configuration owner"));
    }
    owner.end.checked_sub(owner.begin)
}
fn conditional_partition(
    original: ConfigurationBound,
    transformed: ConfigurationBound,
    condition: crate::song::source_uses::FrozenStaticCondition,
    owner: crate::pattern::TimeSpan,
    remaining: &mut u32,
) -> Result<ConfigurationBound, Failure> {
    use crate::song::source_uses::FrozenStaticCondition as C;
    density_charge(remaining, 1)?;
    density_owner_width(owner)?;
    let period = match condition {
        C::Every { period } if period <= 0 => return Ok(original),
        C::Every { period: 1 } => return Ok(transformed),
        C::Every { period } => period,
        C::WhenMod { modulus, .. } if modulus <= 0 => return Ok(original),
        C::WhenMod { threshold, .. } if threshold <= 0 => return Ok(transformed),
        C::WhenMod { modulus, threshold } if threshold >= modulus => return Ok(original),
        C::WhenMod { modulus, .. } => modulus,
        C::Chunk { divisions } if divisions <= 0 => return Ok(original),
        C::Chunk { divisions: 1 } => return Ok(transformed),
        C::Chunk { .. } => {
            // Retimed child anchors need not form complementary masks.
            // Keep independent-child instantaneous ownership and a cycle bound.
            let sum = original.add(transformed)?;
            let per_cycle = sum.components_intersecting(Ratio64::from_int(3))?;
            let births = UniformBirthBound {
                slope: integer(per_cycle)?,
                burst: per_cycle.checked_mul(2).ok_or_else(capacity_overflow)?,
            };
            let instantaneous = sum.instantaneous;
            let finite_total = instantaneous
                .checked_add(births.at(owner.end.checked_sub(owner.begin)?)?)
                .ok_or_else(capacity_overflow)?;
            return Ok(ConfigurationBound {
                instantaneous,
                births,
                finite_total,
            });
        }
    };
    let opens = original
        .instantaneous
        .checked_add(transformed.instantaneous)
        .ok_or_else(capacity_overflow)?;
    let periods = period_ceiling(owner.end.checked_sub(owner.begin)?, period)?
        .checked_add(2)
        .ok_or_else(capacity_overflow)?;
    let finite_total = original
        .finite_total
        .checked_add(transformed.finite_total)
        .and_then(|n| {
            opens
                .checked_mul(periods)
                .and_then(|openings| n.checked_add(openings))
        })
        .ok_or_else(capacity_overflow)?;
    let births = original
        .births
        .add_finite(transformed.births, finite_total)?
        .add_finite(
            UniformBirthBound {
                slope: integer(opens)?.checked_div(Ratio64::from_int(period))?,
                burst: opens,
            },
            finite_total,
        )?;
    let instantaneous = original.instantaneous.max(transformed.instantaneous);
    Ok(ConfigurationBound {
        instantaneous,
        births,
        finite_total,
    })
}
fn finite_repeat(
    child: ConfigurationBound,
    child_duration: Ratio64,
    count: u32,
    owner: crate::pattern::TimeSpan,
    remaining: &mut u32,
) -> Result<ConfigurationBound, Failure> {
    density_charge(remaining, 1)?;
    density_owner_width(owner)?;
    if child_duration < Ratio64::ZERO {
        return Err(Failure::new(
            FailCode::Type,
            "negative repeated configuration duration",
        ));
    }
    if count == 0 || child_duration == Ratio64::ZERO {
        return Ok(ConfigurationBound::empty());
    }
    if count == 1 {
        return Ok(child);
    }
    // A window meets at most ceil(L/D)+1 copies. Count complete components
    // per copy; this deliberately preserves copy identity at touching endpoints.
    Ok(ConfigurationBound {
        instantaneous: child.instantaneous,
        births: UniformBirthBound {
            slope: integer(child.finite_total)?.checked_div(child_duration)?,
            burst: child
                .finite_total
                .checked_mul(2)
                .ok_or_else(capacity_overflow)?,
        },
        finite_total: child
            .finite_total
            .checked_mul(u64::from(count))
            .ok_or_else(capacity_overflow)?,
    })
}

fn integer(value: u64) -> Result<Ratio64, Failure> {
    Ok(Ratio64::from_int(
        i64::try_from(value).map_err(|_| capacity_overflow())?,
    ))
}
fn scalar_ceiling(numerator: i128, denominator: i128) -> Result<u64, Failure> {
    if numerator < 0 || denominator <= 0 {
        return Err(capacity_overflow());
    }
    let value = (numerator / denominator)
        .checked_add(i128::from(numerator % denominator != 0))
        .ok_or_else(capacity_overflow)?;
    u64::try_from(value).map_err(|_| capacity_overflow())
}
fn ceiling(value: Ratio64) -> Result<u64, Failure> {
    scalar_ceiling(i128::from(value.num()), i128::from(value.den()))
}
fn period_ceiling(length: Ratio64, period: i64) -> Result<u64, Failure> {
    scalar_ceiling(
        i128::from(length.num()),
        i128::from(length.den())
            .checked_mul(i128::from(period))
            .ok_or_else(capacity_overflow)?,
    )
}
fn density_clip(
    window: crate::pattern::TimeSpan,
    duration: Ratio64,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    density_owner_width(window)?;
    if duration < Ratio64::ZERO {
        return Err(Failure::new(
            FailCode::Type,
            "negative configuration duration",
        ));
    }
    if duration == Ratio64::ZERO || window.end < Ratio64::ZERO || window.begin >= duration {
        return Ok(None);
    }
    if window.is_point() {
        return Ok((window.begin >= Ratio64::ZERO).then_some(window));
    }
    let begin = window.begin.max(Ratio64::ZERO);
    let end = window.end.min(duration);
    if begin >= end {
        Ok(None)
    } else {
        Ok(Some(crate::pattern::TimeSpan::new(begin, end)?))
    }
}
#[derive(Clone, Copy)]
struct DensitySelection<'a> {
    track: KwId,
    original_members: &'a [FrozenSound],
}
pub(super) fn source_density(
    inventory: &FrozenRoutingInventory,
    pattern: &crate::song::snapshot::FrozenPattern,
    owner: crate::pattern::TimeSpan,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<ConfigurationBound, Failure> {
    density_owner_width(owner)?;
    DensityWalk {
        inventory,
        remaining,
        max_depth,
    }
    .node(pattern, pattern.source_uses.root, owner, None, 0)
}
struct DensityWalk<'a, 'b> {
    inventory: &'a FrozenRoutingInventory,
    remaining: &'b mut u32,
    max_depth: u32,
}

#[cfg(test)]
mod density_tests {
    use super::*;
    fn frozen_routes(code: &str) -> crate::song::PreparedSong {
        use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
        let factory = DecodedSongAssetFactory::new(
            std::collections::BTreeMap::from([(
                "./drum.wav".into(),
                std::sync::Arc::new(crate::host::caps::SampleData {
                    rate: 48000,
                    channels: 2,
                    frames: vec![0.1, 0.1, 0.2, 0.2].into(),
                }),
            )]),
            Default::default(),
            Default::default(),
        );
        let cx = crate::session::song::CandidateBuildCtx {
            assets: &factory,
            asset_limits: SongAssetLimits {
                max_resources: 256,
                max_pcm_bytes: 16_000_000,
                max_source_files: 64,
                max_source_bytes: 1_000_000,
                max_banks: 64,
                max_walk_nodes: 100_000,
                max_walk_depth: 256,
            },
            lock: None,
            cache: None,
        };
        crate::song::prepare_song(
            crate::session::song::evaluate_song_candidate(
                code,
                "routes.vact",
                1,
                crate::song::SnapshotEpoch(100),
                &cx,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn selected_density(song: &crate::song::PreparedSong) -> ConfigurationBound {
        use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
        let inventory = song.snapshot().routing();
        let payload = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .unwrap();
        let mut remaining = 1_000_000;
        source_density(
            inventory,
            payload,
            crate::pattern::TimeSpan::new(Ratio64::ZERO, song.snapshot().duration()).unwrap(),
            &mut remaining,
            256,
        )
        .unwrap()
    }
    #[test]
    fn uniform_density_covers_lazy_huge_repeat_rates_and_weighted_cycle_bursts() {
        use crate::pattern::TimeSpan;
        for expression in [
            "fast p 2",
            "slow p 2",
            "[{hold p 1/1000} nil nil]",
            "stack [{fast p 4} {slow p 2}]",
            "rev p",
            "off p 1/4 {p -> fast p 1}",
            "ply p 4",
        ] {
            let code = format!("let base {{part [drums: {{s :analog}}] duration: 1}}\nlet repeated {{part-repeat base 1000000000}}\nlet developed {{transform-instrument repeated :drums :analog {{p -> {expression}}}}}\nsong developed > play-song");
            let mut song = frozen_routes(&code);
            let density = selected_density(&song);
            assert!(
                density.components_intersecting(Ratio64::ONE).unwrap() < 1000,
                "{expression}"
            );
            for quarter in 0..8 {
                let begin = Ratio64::new(quarter, 4).unwrap();
                let end = begin.checked_add(Ratio64::new(1, 4).unwrap()).unwrap();
                let events = song
                    .query(
                        TimeSpan::new(begin, end).unwrap(),
                        &crate::song::SongLimits::default(),
                    )
                    .unwrap();
                let mut placements = Vec::new();
                for event in events {
                    let origin = event.source_origin.unwrap();
                    let key = (origin.handle.placement().clone(), origin.entry_trace);
                    if !placements.contains(&key) {
                        placements.push(key);
                    }
                }
                assert!(
                    placements.len() as u64
                        <= density
                            .components_intersecting(end.checked_sub(begin).unwrap())
                            .unwrap(),
                    "{expression}, quarter={quarter}"
                );
            }
        }
    }
    #[test]
    fn rates_are_exact_and_nonpositive_windows_overflow_are_explicit() {
        let make = |expression: &str| {
            frozen_routes(&format!("let p0 {{part [drums: {{s :analog}}] duration: 1}}\nlet r {{part-repeat p0 1000000000}}\nsong {{transform-instrument r :drums :analog {{p -> {expression}}}}} > play-song"))
        };
        let fast = selected_density(&make("fast p 2"));
        let slow = selected_density(&make("slow p 2"));
        assert_eq!(fast.births.slope, Ratio64::from_int(2));
        assert_eq!(slow.births.slope, Ratio64::new(1, 2).unwrap());
        assert!(fast.components_intersecting(Ratio64::from_int(-1)).is_err());
        assert!(fast
            .components_intersecting(Ratio64::from_int(i64::MAX))
            .is_err());
        assert_eq!(
            ConfigurationBound::empty()
                .components_intersecting(Ratio64::ZERO)
                .unwrap(),
            0
        );
        assert!(ConfigurationBound {
            instantaneous: u64::MAX,
            births: UniformBirthBound::default(),
            finite_total: u64::MAX
        }
        .scale(2)
        .is_err());
    }
    #[test]
    fn weighted_silent_slots_charge_output_reentry_with_constant_source_configuration() {
        let song = frozen_routes("let p0 {part [drums: {s :analog > slow 64}] duration: 64}\nsong {transform-instrument p0 :drums :analog {p -> [{hold p 1/1000} nil nil]}} > play-song");
        let density = selected_density(&song);
        assert!(density.births.slope > Ratio64::ZERO);
        assert!(
            density
                .components_intersecting(Ratio64::from_int(64))
                .unwrap()
                >= 64
        );
    }
}

#[cfg(test)]
mod configuration_bound_tests {
    use super::*;
    use crate::song::source_uses::FrozenStaticCondition as C;
    fn owner(end: i64) -> crate::pattern::TimeSpan {
        crate::pattern::TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(end)).unwrap()
    }
    #[test]
    fn point_tail_and_periodic_partitions_keep_separate_bounds() {
        let mut work = 100;
        let base = ConfigurationBound::constant(1);
        let every = conditional_partition(base, base, C::Every { period: 2 }, owner(64), &mut work)
            .unwrap();
        assert_eq!(every.live_generations(Ratio64::ZERO).unwrap(), 1);
        assert_eq!(every.births.slope, Ratio64::ONE);
        assert_eq!(every.live_generations(Ratio64::from_int(2)).unwrap(), 7);
        assert_eq!(
            conditional_partition(base, base, C::Every { period: 1 }, owner(64), &mut work)
                .unwrap(),
            base
        );
        assert_eq!(
            conditional_partition(
                base,
                base,
                C::WhenMod {
                    modulus: 3,
                    threshold: 0
                },
                owner(64),
                &mut work
            )
            .unwrap(),
            base
        );
        assert_eq!(
            conditional_partition(
                base,
                base,
                C::WhenMod {
                    modulus: 0,
                    threshold: 1
                },
                owner(64),
                &mut work
            )
            .unwrap(),
            base
        );
        assert_eq!(
            base.scale(3)
                .unwrap()
                .components_intersecting(Ratio64::ZERO)
                .unwrap(),
            3
        );
    }
    #[test]
    fn finite_repeats_cap_counts_without_expanding_or_resetting_work() {
        let mut work = 3;
        let base = ConfigurationBound::constant(1);
        let repeated = finite_repeat(
            base,
            Ratio64::from_int(4),
            1_000_000_000,
            owner(4_000_000_000),
            &mut work,
        )
        .unwrap();
        assert_eq!(repeated.instantaneous, 1);
        assert_eq!(repeated.finite_total, 1_000_000_000);
        assert_eq!(repeated.live_generations(Ratio64::ONE).unwrap(), 4);
        assert_eq!(
            finite_repeat(base, Ratio64::ONE, 0, owner(0), &mut work).unwrap(),
            ConfigurationBound::empty()
        );
        assert_eq!(
            finite_repeat(base, Ratio64::ONE, 1, owner(1), &mut work).unwrap(),
            base
        );
        assert_eq!(
            finite_repeat(base, Ratio64::ONE, 2, owner(2), &mut work)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        assert_eq!(
            base.components_intersecting(Ratio64::from_int(-1))
                .unwrap_err()
                .code,
            FailCode::Type
        );
    }
    #[test]
    fn degenerate_shortcuts_validate_owners_and_initial_births() {
        let mut work = 20;
        let base = ConfigurationBound::constant(2);
        assert_eq!(base.births.at(Ratio64::ONE).unwrap(), 2);
        assert_eq!(base.components_intersecting(Ratio64::ONE).unwrap(), 2);
        let reversed = crate::pattern::TimeSpan {
            begin: Ratio64::ONE,
            end: Ratio64::ZERO,
        };
        assert_eq!(
            conditional_partition(base, base, C::Every { period: 1 }, reversed, &mut work)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        assert_eq!(
            finite_repeat(base, Ratio64::ONE, 0, reversed, &mut work)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        assert_eq!(
            finite_repeat(base, Ratio64::from_int(-1), 0, owner(0), &mut work)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        let point = crate::pattern::TimeSpan::point(Ratio64::new(1, 2).unwrap());
        let repeated = finite_repeat(base, Ratio64::ONE, 2, point, &mut work).unwrap();
        assert_eq!(repeated.components_intersecting(Ratio64::ZERO).unwrap(), 2);
        assert_eq!(
            finite_repeat(base, Ratio64::ZERO, 2, point, &mut work).unwrap(),
            ConfigurationBound::empty()
        );
        let mut huge = base.births;
        huge.burst = u64::MAX;
        assert_eq!(
            huge.add_finite(base.births, 2).unwrap_err().code,
            FailCode::Overflow
        );
        let fragmented = base.fragmented(owner(8), Ratio64::from_int(3)).unwrap();
        assert_eq!(fragmented.live_generations(Ratio64::ZERO).unwrap(), 2);
    }
}
