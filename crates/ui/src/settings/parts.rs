use adw::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::StatsSnapshot;

pub struct Diagnostics {
    expander: adw::ExpanderRow,
    captured: adw::ActionRow,
    processed: adw::ActionRow,
    underruns: adw::ActionRow,
    trims: adw::ActionRow,
    warnings: adw::ActionRow,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnostics {
    pub fn new() -> Self {
        let captured = counter_row(t("Captured frames"));
        let processed = counter_row(t("Processed frames"));
        let underruns = counter_row(t("Underrun samples"));
        let trims = counter_row(t("Drift trims"));
        let warnings = adw::ActionRow::new();
        warnings.set_title(t("Messages"));
        warnings.set_subtitle(t("None"));
        warnings.set_subtitle_lines(4);

        let expander = adw::ExpanderRow::new();
        expander.set_title(t("Diagnostics"));
        expander.set_subtitle(t("Audio counters and engine messages"));
        expander.add_row(&captured);
        expander.add_row(&processed);
        expander.add_row(&underruns);
        expander.add_row(&trims);
        expander.add_row(&warnings);

        Self {
            expander,
            captured,
            processed,
            underruns,
            trims,
            warnings,
        }
    }

    pub fn row(&self) -> &adw::ExpanderRow {
        &self.expander
    }

    pub fn expand(&self) {
        self.expander.set_expanded(true);
    }

    pub fn update(&self, stats: &StatsSnapshot, warnings: &[String]) {
        self.captured
            .set_subtitle(&stats.captured_frames.to_string());
        self.processed
            .set_subtitle(&stats.processed_frames.to_string());
        self.underruns
            .set_subtitle(&stats.underrun_samples.to_string());
        self.trims.set_subtitle(&stats.trim_events.to_string());
        self.warnings.set_subtitle(&if warnings.is_empty() {
            t("None").to_owned()
        } else {
            warnings.join("\n")
        });
    }
}

fn counter_row(title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle("0");
    row
}

pub fn device_labels(devices: &[(String, String)]) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    devices
        .iter()
        .map(|(name, description)| {
            let description = if description.is_empty() {
                name.as_str()
            } else {
                description.as_str()
            };
            let repeated = devices
                .iter()
                .filter(|(_, other)| other.as_str() == description)
                .count()
                > 1;
            if !repeated {
                return description.to_owned();
            }
            let position = seen
                .iter()
                .filter(|candidate| **candidate == description)
                .count()
                + 1;
            seen.push(description);
            format!("{description} ({position})")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, description: &str) -> (String, String) {
        (name.to_owned(), description.to_owned())
    }

    #[test]
    fn unique_descriptions_are_used_as_labels() {
        let labels = device_labels(&[
            pair("alsa_output.a", "Built-in Audio"),
            pair("bluez.1", "Headset"),
        ]);
        assert_eq!(labels, vec!["Built-in Audio", "Headset"]);
    }

    #[test]
    fn repeated_descriptions_are_numbered() {
        let labels = device_labels(&[
            pair("alsa_output.a", "Built-in Audio Pro"),
            pair("alsa_output.b", "Built-in Audio Pro"),
        ]);
        assert_eq!(
            labels,
            vec!["Built-in Audio Pro (1)", "Built-in Audio Pro (2)"]
        );
    }

    #[test]
    fn a_missing_description_falls_back_to_the_name() {
        let labels = device_labels(&[pair("node.one", "")]);
        assert_eq!(labels, vec!["node.one"]);
    }
}
