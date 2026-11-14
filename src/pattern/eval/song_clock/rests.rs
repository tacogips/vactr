//! Keep playback-only projection temporaries off recursive legacy query frames.
use super::CanonicalClockFrame;
use crate::pattern::query::sect;
use crate::vm::fail::Failure;

impl super::super::QState<'_, '_> {
    #[inline(never)]
    pub(super) fn with_rest_frame<T>(
        &mut self,
        frame: impl FnOnce() -> Result<CanonicalClockFrame, Failure>,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let frame = frame()?;
        let mark = self.rests.events.len();
        let result = f(self);
        let mut projected = Vec::new();
        if result.is_ok() {
            for mut event in self.rests.events.drain(mark..) {
                event.whole = event.whole.map(|w| frame.whole(w)).transpose()?;
                let part = frame.whole(event.part)?;
                if let Some(part) = sect(part, frame.parent_piece) {
                    event.part = part;
                    projected.push(event);
                }
            }
        } else {
            self.rests.events.truncate(mark);
        }
        self.rests.events.extend(projected);
        result
    }
}
