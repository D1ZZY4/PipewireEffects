use pipewire_effects_core::{ParamId, ParamSpec};

pub struct Section {
    pub title: String,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandIndices {
    pub enabled: usize,
    pub shape: usize,
    pub frequency: usize,
    pub gain: usize,
    pub quality: usize,
}

pub const MAX_BANDS: usize = 16;

pub fn group_parameters(specs: &[ParamSpec]) -> Vec<Section> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();

    for (index, spec) in specs.iter().enumerate() {
        let prefix = spec
            .id
            .as_str()
            .split('_')
            .next()
            .unwrap_or(spec.id.as_str());
        match groups.iter_mut().find(|(name, _)| name == prefix) {
            Some((_, indices)) => indices.push(index),
            None => groups.push((prefix.to_owned(), vec![index])),
        }
    }

    groups
        .into_iter()
        .map(|(prefix, indices)| Section {
            title: if indices.len() == specs.len() {
                String::from("Parameters")
            } else {
                heading_for(&prefix)
            },
            indices,
        })
        .collect()
}

pub fn heading_for(prefix: &str) -> String {
    let digits: String = prefix
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let words = &prefix[..prefix.len() - digits.len()];

    let mut heading = String::with_capacity(prefix.len() + 1);
    let mut characters = words.chars();
    if let Some(first) = characters.next() {
        heading.extend(first.to_uppercase());
        heading.push_str(characters.as_str());
    }
    if digits.is_empty() {
        heading
    } else {
        format!("{heading} {digits}")
    }
}

pub fn detect_bands(specs: &[ParamSpec]) -> Option<Vec<BandIndices>> {
    let mut bands = Vec::new();

    for band in 1..=MAX_BANDS {
        let prefix = format!("band{band}_");
        let Some(enabled) = find(specs, &format!("{prefix}enabled")) else {
            break;
        };
        let shape = find(specs, &format!("{prefix}type"))?;
        let frequency = find(specs, &format!("{prefix}frequency"))?;
        let gain = find(specs, &format!("{prefix}gain"))?;
        let quality = find(specs, &format!("{prefix}q"))?;

        bands.push(BandIndices {
            enabled,
            shape,
            frequency,
            gain,
            quality,
        });
    }

    (bands.len() >= 2).then_some(bands)
}

pub fn preamp_index(specs: &[ParamSpec]) -> Option<usize> {
    find(specs, "preamp")
}

fn find(specs: &[ParamSpec], id: &str) -> Option<usize> {
    specs.iter().position(|spec| spec.id == *id)
}

#[allow(dead_code)]
pub fn band_id(band: usize, suffix: &str) -> ParamId {
    ParamId::new(format!("band{}_{suffix}", band + 1))
}

#[cfg(test)]
mod tests;
