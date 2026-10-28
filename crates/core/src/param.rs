//! Parameters: the contract between an effect, the user interface and presets.
//!
//! An effect never exposes a bespoke Rust API to the interface. Instead it
//! declares a list of [`ParamSpec`]s — pure metadata — and stores the live
//! values in [`AtomicParam`] cells. That split is what makes the interface
//! generic: it can build a control for *any* effect without knowing what the
//! effect does.
//!
//! Values are kept in atomics rather than behind a mutex so that the interface
//! thread and the realtime audio thread can share them without ever blocking
//! each other. The audio thread reads the current value once per block and
//! smooths it internally.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::{CoreError, Result};

/// The stable identifier of a parameter within an effect.
///
/// Identifiers appear in preset files, so they are part of the public contract
/// and must not be renamed casually.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParamId(String);

impl ParamId {
    /// Creates an identifier from anything string-like.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ParamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ParamId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ParamId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl PartialEq<str> for ParamId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

/// The unit a parameter is expressed in, used for display and for picking a
/// sensible slider scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParamUnit {
    /// A dimensionless number.
    #[default]
    None,
    /// Decibels, the unit for every level in the application.
    Decibel,
    /// Hertz.
    Hertz,
    /// A percentage, stored in the `0.0..=100.0` range.
    Percent,
    /// Seconds.
    Seconds,
    /// Milliseconds.
    Milliseconds,
    /// A compression ratio, displayed as `n:1`.
    Ratio,
    /// A filter Q factor.
    Quality,
}

impl ParamUnit {
    /// The suffix appended after the formatted number, including a leading
    /// space where one is conventional.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Decibel => " dB",
            Self::Hertz => " Hz",
            Self::Percent => " %",
            Self::Seconds => " s",
            Self::Milliseconds => " ms",
            Self::Ratio => ":1",
            Self::Quality => "",
        }
    }
}

/// How many decimal places a value should be shown with.
#[must_use]
const fn decimals_for(step: f32) -> usize {
    if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else {
        2
    }
}

/// Formats a Hertz value, switching to kilohertz once the number gets long.
#[must_use]
fn format_hertz(value: f32) -> String {
    if value >= 1_000.0 {
        format!("{:.2} kHz", value / 1_000.0)
    } else {
        format!("{value:.0} Hz")
    }
}

/// Rounds a value onto the step grid declared by its parameter.
#[must_use]
fn snap_to_step(value: f32, step: f32) -> f32 {
    if step <= 0.0 {
        value
    } else {
        (value / step).round() * step
    }
}

/// The type and range of a parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    /// A continuous value.
    Float {
        /// Inclusive lower bound.
        min: f32,
        /// Inclusive upper bound.
        max: f32,
        /// The value the effect starts with.
        default: f32,
        /// The smallest increment the interface exposes.
        step: f32,
        /// How the value is displayed.
        unit: ParamUnit,
        /// When true the interface maps the slider logarithmically, which is
        /// what you want for frequency and time constants.
        logarithmic: bool,
    },
    /// A two-state toggle.
    Bool {
        /// The value the effect starts with.
        default: bool,
    },
    /// A choice from a fixed list, stored as an index into `options`.
    Enum {
        /// The selectable labels, in display order.
        options: Vec<String>,
        /// The index the effect starts with.
        default: usize,
    },
}

impl ParamKind {
    /// The value an effect starts with.
    #[must_use]
    pub fn default_value(&self) -> ParamValue {
        match self {
            Self::Float { default, .. } => ParamValue::Float(*default),
            Self::Bool { default } => ParamValue::Bool(*default),
            Self::Enum { default, .. } => ParamValue::Enum(*default),
        }
    }

    /// Whether a modal value of the right variant was supplied.
    #[must_use]
    pub fn accepts(&self, value: ParamValue) -> bool {
        matches!(
            (self, value),
            (Self::Float { .. }, ParamValue::Float(_))
                | (Self::Bool { .. }, ParamValue::Bool(_))
                | (Self::Enum { .. }, ParamValue::Enum(_))
        )
    }

    /// The variant name, for error messages.
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::Float { .. } => "float",
            Self::Bool { .. } => "boolean",
            Self::Enum { .. } => "enumeration",
        }
    }

    /// Whether the interface should map the control logarithmically.
    #[must_use]
    pub const fn logarithmic(&self) -> bool {
        matches!(
            self,
            Self::Float {
                logarithmic: true,
                ..
            }
        )
    }

    /// The increment the interface should use. Discrete parameters report
    /// `1.0` so that they land on whole values.
    #[must_use]
    pub fn step(&self) -> f32 {
        match self {
            Self::Float { step, .. } => *step,
            Self::Bool { .. } | Self::Enum { .. } => 1.0,
        }
    }

    /// The declared bounds, if the parameter is continuous.
    #[must_use]
    pub const fn bounds(&self) -> Option<(f32, f32)> {
        match self {
            Self::Float { min, max, .. } => Some((*min, *max)),
            Self::Bool { .. } | Self::Enum { .. } => None,
        }
    }

    /// The number of choices, if the parameter is an enumeration.
    #[must_use]
    pub fn option_count(&self) -> Option<usize> {
        match self {
            Self::Enum { options, .. } => Some(options.len()),
            _ => None,
        }
    }

    /// Clamps a raw number into the range this parameter accepts.
    #[must_use]
    pub fn clamp_f32(&self, value: f32) -> f32 {
        match self {
            Self::Float { min, max, .. } => value.clamp(*min, *max),
            Self::Bool { .. } => {
                if value >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Enum { options, .. } => {
                let last = options.len().saturating_sub(1);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = value.max(0.0) as usize;
                #[allow(clippy::cast_precision_loss)]
                {
                    index.min(last) as f32
                }
            }
        }
    }

    /// Maps a value onto the `0.0..=1.0` range a slider works in.
    #[must_use]
    pub fn normalise(&self, value: f32) -> f64 {
        match self {
            Self::Float {
                min,
                max,
                logarithmic,
                ..
            } => {
                let span = max - min;
                if span <= 0.0 {
                    return 0.0;
                }
                if *logarithmic && *min > 0.0 {
                    let low = f64::from(*min).ln();
                    let high = f64::from(*max).ln();
                    ((f64::from(value.max(0.0).max(*min)).ln() - low) / (high - low))
                        .clamp(0.0, 1.0)
                } else {
                    f64::from((value - min) / span).clamp(0.0, 1.0)
                }
            }
            Self::Bool { .. } => f64::from(value >= 0.5),
            Self::Enum { options, .. } => {
                let last = options.len().saturating_sub(1);
                if last == 0 {
                    0.0
                } else {
                    #[allow(clippy::cast_precision_loss)]
                    let position = f64::from(value.max(0.0) / last as f32);
                    position.clamp(0.0, 1.0)
                }
            }
        }
    }

    /// The inverse of [`ParamKind::normalise`].
    #[must_use]
    pub fn denormalise(&self, position: f64) -> f32 {
        let position = position.clamp(0.0, 1.0);
        match self {
            Self::Float {
                min,
                max,
                logarithmic,
                ..
            } => {
                if *logarithmic && *min > 0.0 {
                    let low = f64::from(*min).ln();
                    let high = f64::from(*max).ln();
                    #[allow(clippy::cast_possible_truncation)]
                    let value = (low + position * (high - low)).exp() as f32;
                    value.clamp(*min, *max)
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    let value = (*min as f64 + position * f64::from(*max - *min)) as f32;
                    value.clamp(*min, *max)
                }
            }
            Self::Bool { .. } => {
                if position >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Enum { options, .. } => {
                let last = options.len().saturating_sub(1);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = (position * last as f64).round() as usize;
                #[allow(clippy::cast_precision_loss)]
                {
                    index.min(last) as f32
                }
            }
        }
    }

    /// Renders a value the way the user should see it.
    #[must_use]
    pub fn format_value(&self, value: ParamValue) -> String {
        match (self, value) {
            (Self::Float { unit, step, .. }, ParamValue::Float(raw)) => {
                let snapped = snap_to_step(raw, *step);
                match unit {
                    ParamUnit::Hertz => format_hertz(snapped),
                    ParamUnit::Ratio => format!("{snapped:.1}{}", unit.suffix()),
                    ParamUnit::None => format!("{snapped:.prec$}", prec = decimals_for(*step)),
                    _ => format!(
                        "{snapped:.prec$}{}",
                        unit.suffix(),
                        prec = decimals_for(*step)
                    ),
                }
            }
            (Self::Bool { .. }, ParamValue::Bool(raw)) => {
                (if raw { "On" } else { "Off" }).to_owned()
            }
            (Self::Enum { options, .. }, ParamValue::Enum(index)) => options
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("#{index}")),
            (_, other) => format!("{other:?}"),
        }
    }
}

/// A concrete parameter value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    /// A continuous value.
    Float(f32),
    /// A toggle state.
    Bool(bool),
    /// An index into the parameter's option list.
    Enum(usize),
}

impl ParamValue {
    /// The value as a float, using the convention that booleans map to
    /// `0.0`/`1.0` and enumeration indices map to their index.
    #[must_use]
    pub fn to_f32(self) -> f32 {
        match self {
            Self::Float(value) => value,
            Self::Bool(value) => {
                if value {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Enum(index) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    index as f32
                }
            }
        }
    }

    /// The value as a boolean, using `>= 0.5` as the threshold for numbers.
    #[must_use]
    pub fn to_bool(self) -> bool {
        match self {
            Self::Bool(value) => value,
            Self::Float(value) => value >= 0.5,
            Self::Enum(index) => index != 0,
        }
    }

    /// The value as an index, truncating floats.
    #[must_use]
    pub fn to_index(self) -> usize {
        match self {
            Self::Enum(index) => index,
            Self::Float(value) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    value.max(0.0) as usize
                }
            }
            Self::Bool(value) => usize::from(value),
        }
    }

    /// Whether the value is a usable number.
    #[must_use]
    pub fn is_finite(self) -> bool {
        match self {
            Self::Float(value) => value.is_finite(),
            Self::Bool(_) | Self::Enum(_) => true,
        }
    }

    /// The variant name, for error messages.
    #[must_use]
    pub const fn variant_name(self) -> &'static str {
        match self {
            Self::Float(_) => "float",
            Self::Bool(_) => "boolean",
            Self::Enum(_) => "enumeration",
        }
    }
}

/// The complete description of one parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// Stable identifier, referenced by presets.
    pub id: ParamId,
    /// Short label for the interface.
    pub label: String,
    /// Type, range and unit.
    pub kind: ParamKind,
    /// One-line explanation, shown as a tooltip.
    pub doc: String,
}

impl ParamSpec {
    /// Builds a specification from its parts.
    pub fn new(id: impl Into<ParamId>, label: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            doc: String::new(),
        }
    }

    /// Builds a continuous parameter.
    pub fn float(
        id: impl Into<ParamId>,
        label: impl Into<String>,
        min: f32,
        max: f32,
        default: f32,
        step: f32,
        unit: ParamUnit,
    ) -> Self {
        Self::new(
            id,
            label,
            ParamKind::Float {
                min,
                max,
                default: default.clamp(min, max),
                step,
                unit,
                logarithmic: false,
            },
        )
    }

    /// Builds a continuous parameter whose slider is mapped logarithmically.
    ///
    /// `min` must be strictly positive.
    pub fn float_log(
        id: impl Into<ParamId>,
        label: impl Into<String>,
        min: f32,
        max: f32,
        default: f32,
        unit: ParamUnit,
    ) -> Self {
        Self::new(
            id,
            label,
            ParamKind::Float {
                min,
                max,
                default: default.clamp(min, max),
                step: 0.0,
                unit,
                logarithmic: true,
            },
        )
    }

    /// Builds a toggle.
    pub fn boolean(id: impl Into<ParamId>, label: impl Into<String>, default: bool) -> Self {
        Self::new(id, label, ParamKind::Bool { default })
    }

    /// Builds a fixed choice.
    pub fn enumerate(
        id: impl Into<ParamId>,
        label: impl Into<String>,
        options: &[&str],
        default: usize,
    ) -> Self {
        let options: Vec<String> = options.iter().map(|o| (*o).to_owned()).collect();
        let default = default.min(options.len().saturating_sub(1));
        Self::new(id, label, ParamKind::Enum { options, default })
    }

    /// Attaches the tooltip text.
    #[must_use]
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    /// The value this parameter starts with.
    #[must_use]
    pub fn default_value(&self) -> ParamValue {
        self.kind.default_value()
    }
}

/// A parameter value stored in an atomic cell, shareable between threads.
///
/// The cell holds the value as `f32` bits. Booleans are stored as `0.0`/`1.0`
/// and enumeration indices as their index; the accompanying [`ParamSpec`] is
/// what restores the richer type.
#[derive(Debug, Clone)]
pub struct AtomicParam {
    cell: Arc<AtomicU32>,
}

impl AtomicParam {
    /// Creates a cell initialised to `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            cell: Arc::new(AtomicU32::new(value.to_bits())),
        }
    }

    /// Reads the current value.
    ///
    /// Relaxed ordering is sufficient: the value is a scalar that stands alone,
    /// with no other memory that needs to be published alongside it.
    #[must_use]
    pub fn get(&self) -> f32 {
        f32::from_bits(self.cell.load(Ordering::Relaxed))
    }

    /// Writes a new value.
    pub fn set(&self, value: f32) {
        self.cell.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Whether two handles refer to the same underlying cell.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cell, &other.cell)
    }
}

/// A parameter specification paired with its live value.
#[derive(Debug, Clone)]
pub struct Param {
    /// The specification.
    pub spec: ParamSpec,
    /// The live value.
    pub value: AtomicParam,
}

/// The full parameter set of one effect instance.
///
/// Specs and values are kept in the same order, so an index into one is an
/// index into the other. The interface relies on that to build controls.
#[derive(Debug, Clone)]
pub struct ParamSet {
    specs: Vec<ParamSpec>,
    values: Vec<AtomicParam>,
}

impl ParamSet {
    /// Builds a set from specifications, initialising every value to its
    /// declared default.
    #[must_use]
    pub fn new(specs: Vec<ParamSpec>) -> Self {
        let values = specs
            .iter()
            .map(|spec| AtomicParam::new(spec.default_value().to_f32()))
            .collect();
        Self { specs, values }
    }

    /// The specifications, in declaration order.
    #[must_use]
    pub fn specs(&self) -> &[ParamSpec] {
        &self.specs
    }

    /// The live values, in declaration order.
    #[must_use]
    pub fn values(&self) -> &[AtomicParam] {
        &self.values
    }

    /// The number of parameters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Finds the index of a parameter by identifier.
    #[must_use]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.specs.iter().position(|spec| spec.id.as_str() == id)
    }

    /// The specification at `index`.
    #[must_use]
    pub fn spec(&self, index: usize) -> Option<&ParamSpec> {
        self.specs.get(index)
    }

    /// The value at `index`, typed by its specification.
    #[must_use]
    pub fn value(&self, index: usize) -> Option<ParamValue> {
        let spec = self.specs.get(index)?;
        let raw = self.values.get(index)?.get();
        Some(match spec.kind {
            ParamKind::Float { .. } => ParamValue::Float(raw),
            ParamKind::Bool { .. } => ParamValue::Bool(raw >= 0.5),
            ParamKind::Enum { .. } =>
            {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                ParamValue::Enum(raw.max(0.0) as usize)
            }
        })
    }

    /// The value at `index` as a float.
    #[must_use]
    pub fn value_f32(&self, index: usize) -> f32 {
        self.values.get(index).map_or(0.0, AtomicParam::get)
    }

    /// The value at `index` as a boolean.
    #[must_use]
    pub fn value_bool(&self, index: usize) -> bool {
        self.value_f32(index) >= 0.5
    }

    /// The value at `index` as an enumeration index.
    #[must_use]
    pub fn value_enum(&self, index: usize) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            self.value_f32(index).max(0.0) as usize
        }
    }

    /// Assigns a value, validating it against the specification first.
    pub fn set(&self, index: usize, value: ParamValue) -> Result<()> {
        let spec = self.specs.get(index).ok_or(CoreError::IndexOutOfRange {
            index,
            len: self.specs.len(),
        })?;
        if !spec.kind.accepts(value) {
            return Err(CoreError::ValueTypeMismatch {
                param: spec.id.to_string(),
                expected: spec.kind.variant_name(),
                got: value.variant_name(),
            });
        }
        if matches!(spec.kind, ParamKind::Float { .. }) && !value.is_finite() {
            return Err(CoreError::ValueOutOfRange {
                param: spec.id.to_string(),
                value: value.to_f32(),
                min: spec.kind.bounds().map_or(0.0, |b| b.0),
                max: spec.kind.bounds().map_or(0.0, |b| b.1),
            });
        }
        if let Some(handle) = self.values.get(index) {
            handle.set(spec.kind.clamp_f32(value.to_f32()));
        }
        Ok(())
    }

    /// Assigns a raw float, clamping it to the declared range.
    ///
    /// This is the path the interface uses when a slider moves, so it
    /// deliberately never fails.
    pub fn set_f32(&self, index: usize, value: f32) {
        if let (Some(spec), Some(handle)) = (self.specs.get(index), self.values.get(index)) {
            if value.is_finite() {
                handle.set(spec.kind.clamp_f32(value));
            }
        }
    }

    /// Assigns a value by parameter identifier.
    pub fn set_by_id(&self, id: &str, value: ParamValue) -> Result<()> {
        let index = self
            .index_of(id)
            .ok_or_else(|| CoreError::UnknownParameter {
                effect: String::new(),
                param: id.to_owned(),
            })?;
        self.set(index, value)
    }

    /// Restores every parameter to its declared default.
    pub fn reset_to_defaults(&self) {
        for (spec, handle) in self.specs.iter().zip(&self.values) {
            handle.set(spec.default_value().to_f32());
        }
    }

    /// Snapshots every parameter as `(id, value)` pairs.
    #[must_use]
    pub fn to_pairs(&self) -> Vec<(ParamId, ParamValue)> {
        self.specs
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| self.value(index).map(|value| (spec.id.clone(), value)))
            .collect()
    }

    /// Renders a parameter for display.
    #[must_use]
    pub fn format(&self, index: usize) -> String {
        match (self.specs.get(index), self.value(index)) {
            (Some(spec), Some(value)) => spec.kind.format_value(value),
            _ => String::from("—"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain_spec() -> ParamSpec {
        ParamSpec::float("gain", "Gain", -60.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
    }

    #[test]
    fn defaults_are_applied_on_construction() {
        let set = ParamSet::new(vec![gain_spec()]);
        assert_eq!(set.value(0), Some(ParamValue::Float(0.0)));
    }

    #[test]
    fn float_values_are_clamped_into_range() {
        let set = ParamSet::new(vec![gain_spec()]);
        set.set_f32(0, 1_000.0);
        assert_eq!(set.value_f32(0), 24.0);
        set.set_f32(0, -1_000.0);
        assert_eq!(set.value_f32(0), -60.0);
    }

    #[test]
    fn non_finite_values_are_ignored() {
        let set = ParamSet::new(vec![gain_spec()]);
        set.set_f32(0, 6.0);
        set.set_f32(0, f32::NAN);
        assert_eq!(set.value_f32(0), 6.0);
    }

    #[test]
    fn type_mismatches_are_rejected() {
        let set = ParamSet::new(vec![gain_spec()]);
        let error = set.set(0, ParamValue::Bool(true));
        assert!(matches!(error, Err(CoreError::ValueTypeMismatch { .. })));
    }

    #[test]
    fn unknown_parameter_is_reported() {
        let set = ParamSet::new(vec![gain_spec()]);
        let error = set.set_by_id("nope", ParamValue::Float(1.0));
        assert!(matches!(error, Err(CoreError::UnknownParameter { .. })));
    }

    #[test]
    fn logarithmic_round_trip_is_stable() {
        let spec = ParamSpec::float_log(
            "freq",
            "Frequency",
            20.0,
            20_000.0,
            1_000.0,
            ParamUnit::Hertz,
        );
        for probe in [20.0_f32, 100.0, 1_000.0, 8_000.0, 20_000.0] {
            let position = spec.kind.normalise(probe);
            let back = spec.kind.denormalise(position);
            assert!(
                (back - probe).abs() / probe < 1e-3,
                "round trip failed for {probe}: got {back}"
            );
        }
    }

    #[test]
    fn hertz_is_formatted_with_a_kilohertz_suffix_when_long() {
        let spec = ParamSpec::float(
            "freq",
            "Frequency",
            20.0,
            20_000.0,
            440.0,
            1.0,
            ParamUnit::Hertz,
        );
        assert_eq!(spec.kind.format_value(ParamValue::Float(440.0)), "440 Hz");
        assert_eq!(
            spec.kind.format_value(ParamValue::Float(1_200.0)),
            "1.20 kHz"
        );
    }

    #[test]
    fn enumeration_indices_are_clamped() {
        let spec = ParamSpec::enumerate("mode", "Mode", &["Low", "Mid", "High"], 1);
        assert_eq!(spec.kind.clamp_f32(99.0), 2.0);
        assert_eq!(spec.kind.clamp_f32(-3.0), 0.0);
    }

    #[test]
    fn atomic_params_are_shared_between_handles() {
        let set = ParamSet::new(vec![gain_spec()]);
        let cloned = set.values()[0].clone();
        set.set_f32(0, 3.0);
        assert_eq!(cloned.get(), 3.0);
        assert!(cloned.ptr_eq(&set.values()[0]));
    }
}
