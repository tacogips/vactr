use super::{Inp, MAX_PORTS};
/// Fixed-size collection of kernel inputs.
pub struct PortValues<'a> {
    inputs: [Inp<'a>; MAX_PORTS],
}
impl<'a> PortValues<'a> {
    /// Creates zero-valued inputs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inputs: [Inp::Val(0.0); MAX_PORTS],
        }
    }
    /// Creates inputs from ordered default values.
    #[must_use]
    pub fn from_defaults(defaults: &[(&str, f32)]) -> Self {
        let mut result = Self::new();
        for (i, (_, value)) in defaults.iter().take(MAX_PORTS).enumerate() {
            result.set(i, *value);
        }
        result
    }
    /// Sets a constant input, ignoring out-of-range indices.
    pub fn set(&mut self, index: usize, value: f32) {
        if let Some(input) = self.inputs.get_mut(index) {
            *input = Inp::Val(value);
        }
    }
    /// Sets an audio input, ignoring out-of-range indices.
    pub fn set_buffer(&mut self, index: usize, buffer: &'a [f32]) {
        if let Some(input) = self.inputs.get_mut(index) {
            *input = Inp::Buf(buffer);
        }
    }
    /// Returns all input values.
    #[must_use]
    pub fn inputs(&self) -> &[Inp<'a>; MAX_PORTS] {
        &self.inputs
    }
}
impl Default for PortValues<'_> {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_updates() {
        let mut p = PortValues::from_defaults(&[("freq", 440.0), ("amp", 0.5)]);
        assert_eq!(p.inputs()[0].first(), 440.0);
        assert_eq!(p.inputs()[1].first(), 0.5);
        assert_eq!(p.inputs()[2].first(), 0.0);
        p.set(60, 1.0);
        p.set_buffer(0, &[1.0, 2.0]);
        assert_eq!(p.inputs()[0].at(1), 2.0);
    }
}
