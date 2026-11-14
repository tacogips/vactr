use crate::pattern::pat::PatNode;
use crate::vm::fail::Failure;

impl super::super::QState<'_, '_> {
    pub(crate) fn with_clock_dispatch<T>(
        &mut self,
        node: &PatNode,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        // Exhaustive: every uninstrumented timing change carries a barrier.
        let unsupported = match node {
            PatNode::Chunk(..) => true,
            PatNode::Euclid(..)
            | PatNode::Ply(..)
            | PatNode::Arp(..)
            | PatNode::Chop(..)
            | PatNode::Striate(..)
            | PatNode::LoopAt(..)
            | PatNode::Fit(..)
            | PatNode::Cat(_)
            | PatNode::FastCat(_)
            | PatNode::Off(..)
            | PatNode::Grid(..)
            | PatNode::Segment(..)
            | PatNode::Steps(_)
            | PatNode::Pure(_)
            | PatNode::Sound { .. }
            | PatNode::SongSource(_)
            | PatNode::Signal(_)
            | PatNode::Fast(..)
            | PatNode::Slow(..)
            | PatNode::Hurry(..)
            | PatNode::Rev(_)
            | PatNode::Every(..)
            | PatNode::WhenMod(..)
            | PatNode::Iter(..)
            | PatNode::SometimesBy(..)
            | PatNode::DegradeBy(..)
            | PatNode::Maybe(..)
            | PatNode::Choose(_)
            | PatNode::Hold(..)
            | PatNode::Repeat(..)
            | PatNode::Stack(_)
            | PatNode::Superimpose(..)
            | PatNode::Jux(..)
            | PatNode::Slice { .. }
            | PatNode::Splice { .. }
            | PatNode::Control(..)
            | PatNode::ScaleNotes(..)
            | PatNode::Chord(..)
            | PatNode::Voicing(_)
            | PatNode::Range(..)
            | PatNode::MidiNotes { .. }
            | PatNode::Tune { .. }
            | PatNode::Harp { .. }
            | PatNode::Inversion { .. } => false,
            PatNode::Strum(..) => true,
        };
        if unsupported {
            self.with_clock_unknown(f)
        } else {
            f(self)
        }
    }
}
