use super::ParamDef;
/// Maximum effect parameter count.
pub const MAX_PARAMS: usize = 28;
/// Fixed-capacity clamped parameter values.
pub struct ParamSet {
    defs: &'static [ParamDef],
    values: [f32; MAX_PARAMS],
}
impl ParamSet {
    /// Initializes values from definitions.
    #[must_use]
    pub fn new(defs: &'static [ParamDef]) -> Self {
        let mut values = [0.0; MAX_PARAMS];
        for (value, def) in values.iter_mut().zip(defs) {
            *value = def.default;
        }
        Self { defs, values }
    }
    /// Sets an indexed value after clamping.
    pub fn set(&mut self, index: usize, value: f32) {
        if let (Some(def), Some(slot)) = (self.defs.get(index), self.values.get_mut(index)) {
            *slot = def.clamp(value);
        }
    }
    /// Sets a named value and reports whether it exists within capacity.
    pub fn set_named(&mut self, name: &str, value: f32) -> bool {
        if let Some(index) = self
            .defs
            .iter()
            .take(MAX_PARAMS)
            .position(|d| d.name == name)
        {
            self.set(index, value);
            true
        } else {
            false
        }
    }
    /// Returns defined values.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values[..self.defs.len().min(MAX_PARAMS)]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    static DEFS: &[ParamDef] = &[ParamDef::new("drive", 0.5, 0.0, 1.0)];
    #[test]
    fn defaults_clamp_and_named() {
        let mut p = ParamSet::new(DEFS);
        assert_eq!(p.values(), &[0.5]);
        p.set(0, 2.0);
        assert_eq!(p.values(), &[1.0]);
        p.set(0, f32::NAN);
        assert_eq!(p.values(), &[0.5]);
        p.set(5, 1.0);
        assert_eq!(p.values(), &[0.5]);
        assert!(p.set_named("drive", 0.25));
        assert_eq!(p.values(), &[0.25]);
        assert!(!p.set_named("x", 1.0));
    }
}
