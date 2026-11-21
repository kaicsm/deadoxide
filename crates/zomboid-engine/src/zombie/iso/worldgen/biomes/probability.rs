//! zombie.iso.worldgen.utils.probabilities: the probability of a biome feature - a literal double, or a
//! SandboxOptions double option by name.

use crate::zombie::sandbox_options::{SandboxOptions, Value};

/// zombie.iso.worldgen.utils.probabilities.Probability and its two implementations, ProbaDouble and ProbaString.
/// The game only reads `getValue()` (a float); which kind it is still matters to a dump, so both are kept.
#[derive(Clone, Copy, Debug)]
pub enum Probability {
    /// ProbaDouble: the literal from the Lua.
    Double(f64),
    /// ProbaString: `Sandbox.<option>`, the option's value read when the reader parsed the feature.
    Sandbox(f32),
}

impl Probability {
    /// `new ProbaDouble(Double)`.
    pub fn proba_double(value: f64) -> Self {
        Probability::Double(value)
    }

    /// `new ProbaString(String)`: `"Sandbox.<option>"` reads `SandboxOptions.instance.getOptionByName(field)`
    /// cast to DoubleSandboxOption and its `getValue()`. The port has no statics, so the instance is passed in
    /// (the caller resolves it at the same point the original's lazy static would first run).
    pub fn proba_string(value: &str, sandbox: &SandboxOptions) -> Result<Self, String> {
        let mut parts = value.split('.');
        let clazz = parts.next().unwrap_or("");
        let field = parts.next();
        // `value.split("\\.")[0]` / `[1]`: a value without a '.' is an ArrayIndexOutOfBoundsException.
        let Some(field) = field else {
            return Err(format!(
                "ArrayIndexOutOfBoundsException: ProbaString \"{value}\""
            ));
        };
        if clazz != "Sandbox" {
            return Err(format!("Unknown class type: {clazz}"));
        }
        match sandbox.get(field) {
            // `((DoubleSandboxOption) option).getValue()`
            Some(Value::Double(d)) => Ok(Probability::Sandbox(*d as f32)),
            Some(_) => Err(format!(
                "ClassCastException: class SandboxOptions${field} cannot be cast to DoubleSandboxOption"
            )),
            None => Err(format!("Invalid sandbox option: {field}")),
        }
    }

    /// Probability.getValue().
    pub fn get_value(&self) -> f32 {
        match self {
            Probability::Double(d) => *d as f32,
            Probability::Sandbox(f) => *f,
        }
    }
}
