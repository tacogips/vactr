/// One installed sample as the audio side reads it.
#[derive(Clone, Copy, Debug)]
pub struct SampleView<'a> {
    /// Interleaved frames.
    pub data: &'a [f32],
    /// Number of interleaved channels.
    pub channels: u8,
    /// Sample rate in hertz.
    pub rate: u32,
}

impl SampleView<'_> {
    /// The number of frames.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.data.len() / usize::from(self.channels.max(1))
    }
}

/// Provides sample data by resource identifier.
pub trait SampleSource {
    /// Returns the requested sample, if available.
    fn sample(&self, id: u32) -> Option<SampleView<'_>>;
}

/// A sample source that is always empty.
pub struct NoSamples;

impl SampleSource for NoSamples {
    fn sample(&self, _id: u32) -> Option<SampleView<'_>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_source_and_frames() {
        assert!(NoSamples.sample(0).is_none());
        assert_eq!(
            SampleView {
                data: &[0.0; 6],
                channels: 2,
                rate: 48000
            }
            .frames(),
            3
        );
    }
}
