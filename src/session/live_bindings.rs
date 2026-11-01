//! Successful document evaluations reconcile only their own live sinks.

use std::collections::BTreeSet;

use crate::ns::depgraph::{FormId, WriteItem};
use crate::ns::stage::{EffectSink, SlotKey, StagedEffect};
use crate::reader::span::FileId;
use crate::session::session::Session;

impl Session {
    pub(super) fn reconcile_live_bindings(&mut self, file: FileId, evaluated: &BTreeSet<FormId>) {
        // Runtime acceptance is authoritative even for binds released during
        // tick-time reactive passes or stage_now, outside document evaluation.
        let obsolete: Vec<SlotKey> = self
            .rt
            .slots()
            .iter()
            .filter(|slot| slot.binding_file == Some(file) && !slot.is_idle() && !slot.ephemeral)
            .filter(|slot| {
                self.ev
                    .form_of_bind(slot.key)
                    .is_none_or(|owner| !evaluated.contains(&owner))
            })
            .map(|slot| slot.key)
            .collect();

        // Failed attempted takeovers may own graph writes but no audible slot.
        // Retire those absent sources too, without revoking another file's sound.
        let mut retired = BTreeSet::new();
        for form in self.ev.graph().ids() {
            let Some(record) = self.ev.graph().get(form) else {
                continue;
            };
            if record.node.span.file != file || evaluated.contains(&form) {
                continue;
            }
            for write in &record.writes {
                if let WriteItem::Bind(key) = write {
                    if self.ev.form_of_bind(*key) == Some(form) {
                        retired.insert(*key);
                    }
                }
            }
        }
        for key in retired {
            self.ev.retire_live_binding(key, file);
        }
        if obsolete.is_empty() {
            return;
        }
        for key in obsolete {
            self.sink.apply(StagedEffect::Revoke(key));
        }
        // Existing per-slot stop machinery cancels staged/rebound work and
        // releases voices naturally, without touching other slots or outputs.
        let _ = self.rt.drain(&mut self.ev);
    }
}
