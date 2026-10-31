use std::fmt;

use pwe_core::ParamSet;
use pwe_core::i18n::tr;
use pwe_dsp::FilterKind;
use pwe_dsp::effects::{BAND_COUNT, BAND_FREQUENCIES, BAND_IDS};

pub const DEFAULT_Q: f32 = 1.414;

#[derive(Debug)]
pub struct Report {
    pub applied: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct ImportError {
    line: usize,
    detail: String,
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.detail)
    }
}

fn error(line: usize, detail: String) -> ImportError {
    ImportError { line, detail }
}

enum Command {
    Preamp(f32),
    Graphic(Vec<(f32, f32)>),
    Filter {
        kind: FilterKind,
        frequency: f32,
        gain: f32,
        q: f32,
    },
}

pub fn apply(params: &ParamSet, text: &str) -> Result<Report, ImportError> {
    let mut commands = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        let Some((key, rest)) = trimmed.split_once(':') else {
            return Err(error(line, tr("expected 'Key: value'", &[])));
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "preamp" => {
                let value = number(rest.split_whitespace().next().unwrap_or(""))
                    .ok_or_else(|| error(line, tr("the preamp value is not a number", &[])))?;
                commands.push(Command::Preamp(value));
            }
            "graphiceq" => commands.push(Command::Graphic(parse_graphic_eq(rest, line)?)),
            "filter" => {
                if let Some(command) = parse_filter(rest, line)? {
                    commands.push(command);
                }
            }
            "device" | "channel" => {}
            other => {
                return Err(error(
                    line,
                    tr("the directive '{0}' is not supported", &[other]),
                ));
            }
        }
    }
    for (band, frequency) in BAND_FREQUENCIES.iter().enumerate() {
        set_band(
            params,
            band,
            FilterKind::Peaking,
            *frequency,
            0.0,
            DEFAULT_Q,
        );
    }
    let _ = params.set_f32("output_gain", 0.0);
    let mut report = Report {
        applied: 0,
        warnings: Vec::new(),
    };
    let mut next_filter = 0usize;
    for command in commands {
        match command {
            Command::Preamp(value) => {
                let _ = params.set_f32("output_gain", value);
                report.applied += 1;
            }
            Command::Graphic(points) => {
                if points.len() <= BAND_COUNT {
                    for (band, default_frequency) in BAND_FREQUENCIES.iter().enumerate() {
                        let (frequency, gain) = points
                            .get(band)
                            .copied()
                            .unwrap_or((*default_frequency, 0.0));
                        set_band(
                            params,
                            band,
                            FilterKind::Peaking,
                            frequency,
                            gain,
                            DEFAULT_Q,
                        );
                    }
                } else {
                    report.warnings.push(tr(
                        "GraphicEQ has {0} points and was resampled to {1} bands",
                        &[&points.len().to_string(), &BAND_COUNT.to_string()],
                    ));
                    for (band, frequency) in BAND_FREQUENCIES.iter().enumerate() {
                        let gain = interpolate(&points, *frequency);
                        set_band(
                            params,
                            band,
                            FilterKind::Peaking,
                            *frequency,
                            gain,
                            DEFAULT_Q,
                        );
                    }
                }
                report.applied += 1;
            }
            Command::Filter {
                kind,
                frequency,
                gain,
                q,
            } => {
                if next_filter >= BAND_COUNT {
                    report.warnings.push(tr(
                        "the file has more than {0} filters and the extra ones were ignored",
                        &[&BAND_COUNT.to_string()],
                    ));
                    continue;
                }
                set_band(params, next_filter, kind, frequency, gain, q);
                next_filter += 1;
                report.applied += 1;
            }
        }
    }
    Ok(report)
}

fn number(token: &str) -> Option<f32> {
    let cleaned = token
        .trim()
        .trim_end_matches(|character: char| character.is_ascii_alphabetic());
    cleaned.parse::<f32>().ok()
}

fn kind_index(kind: FilterKind) -> u16 {
    FilterKind::ALL
        .iter()
        .position(|value| *value == kind)
        .expect("the filter kind is published") as u16
}

fn field_id(gain_id: &str, field: &str) -> String {
    format!("{}_{}", gain_id.trim_end_matches("_gain"), field)
}

fn set_band(params: &ParamSet, band: usize, kind: FilterKind, frequency: f32, gain: f32, q: f32) {
    let gain_id = BAND_IDS[band];
    let _ = params.set_f32(gain_id, gain);
    let _ = params.set_choice(&field_id(gain_id, "type"), kind_index(kind));
    let _ = params.set_f32(&field_id(gain_id, "frequency"), frequency);
    let _ = params.set_f32(&field_id(gain_id, "q"), q);
}

fn parse_graphic_eq(rest: &str, line: usize) -> Result<Vec<(f32, f32)>, ImportError> {
    let mut points: Vec<(f32, f32)> = Vec::new();
    for part in rest.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut tokens = part.split_whitespace();
        let frequency = tokens
            .next()
            .and_then(number)
            .ok_or_else(|| error(line, tr("a GraphicEQ point has no frequency", &[])))?;
        let gain = tokens
            .next()
            .and_then(number)
            .ok_or_else(|| error(line, tr("a GraphicEQ point has no gain", &[])))?;
        points.push((frequency, gain));
    }
    if points.is_empty() {
        return Err(error(line, tr("the GraphicEQ line has no points", &[])));
    }
    points.sort_by(|left, right| left.0.total_cmp(&right.0));
    Ok(points)
}

fn interpolate(points: &[(f32, f32)], frequency: f32) -> f32 {
    let first = points[0];
    if frequency <= first.0 {
        return first.1;
    }
    let last = points[points.len() - 1];
    if frequency >= last.0 {
        return last.1;
    }
    for pair in points.windows(2) {
        let (low, low_gain) = pair[0];
        let (high, high_gain) = pair[1];
        if frequency <= high {
            if high <= low {
                return high_gain;
            }
            let position = (frequency / low).ln() / (high / low).ln();
            return low_gain + position * (high_gain - low_gain);
        }
    }
    last.1
}

fn filter_kind(token: &str) -> Option<FilterKind> {
    match token.to_ascii_uppercase().as_str() {
        "PK" | "PEQ" | "MODAL" | "BELL" => Some(FilterKind::Peaking),
        "LS" | "LSC" | "LS6DB" | "LS12DB" => Some(FilterKind::LowShelf),
        "HS" | "HSC" | "HS6DB" | "HS12DB" => Some(FilterKind::HighShelf),
        "LP" | "LPQ" => Some(FilterKind::LowPass),
        "HP" | "HPQ" => Some(FilterKind::HighPass),
        "BP" | "BPQ" => Some(FilterKind::BandPass),
        "NO" | "NOTCH" => Some(FilterKind::Notch),
        "AP" | "ALLPASS" => Some(FilterKind::AllPass),
        _ => None,
    }
}

fn parse_filter(rest: &str, line: usize) -> Result<Option<Command>, ImportError> {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.iter().any(|token| token.eq_ignore_ascii_case("OFF")) {
        return Ok(None);
    }
    let mut kind = None;
    let mut frequency = None;
    let mut gain = 0.0f32;
    let mut q = DEFAULT_Q;
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if let Some(found) = filter_kind(token) {
            kind = Some(found);
            index += 1;
            continue;
        }
        match token.to_ascii_uppercase().as_str() {
            "FC" | "FREQUENCY" => {
                frequency = tokens.get(index + 1).and_then(|value| number(value));
                index += 2;
            }
            "GAIN" => {
                gain = tokens
                    .get(index + 1)
                    .and_then(|value| number(value))
                    .unwrap_or(0.0);
                index += 2;
            }
            "Q" | "QUALITY" => {
                q = tokens
                    .get(index + 1)
                    .and_then(|value| number(value))
                    .unwrap_or(DEFAULT_Q);
                index += 2;
            }
            _ => index += 1,
        }
    }
    let kind = kind.ok_or_else(|| error(line, tr("the filter type is not recognised", &[])))?;
    let frequency = frequency.ok_or_else(|| error(line, tr("the filter has no frequency", &[])))?;
    Ok(Some(Command::Filter {
        kind,
        frequency,
        gain,
        q,
    }))
}

#[cfg(test)]
#[path = "apo_tests.rs"]
mod tests;
