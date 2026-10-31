mod set;
mod spec;
mod value;

pub use set::{ParamHandle, ParamSet};
pub use spec::{FloatParam, ParamKind, ParamSpec, ParamUnit};
pub use value::ParamValue;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
