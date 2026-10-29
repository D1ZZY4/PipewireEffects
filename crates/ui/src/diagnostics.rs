use adw::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::StatsSnapshot;

pub struct Diagnostics {
    expander: adw::ExpanderRow,
    captured: adw::ActionRow,
    processed: adw::ActionRow,
    underruns: adw::ActionRow,
    overruns: adw::ActionRow,
    trims: adw::ActionRow,
    occupancy: adw::ActionRow,
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
        let overruns = counter_row(t("Overrun samples"));
        let trims = counter_row(t("Drift trims"));
        let occupancy = counter_row(t("Buffer occupancy"));
        let warnings = adw::ActionRow::new();
        warnings.set_title(t("Messages"));
        warnings.set_subtitle(t("None"));
        warnings.set_subtitle_lines(4);

        let expander = adw::ExpanderRow::new();
        expander.set_title(t("Engine counters"));
        expander.set_subtitle(t(
            "Audio counters and engine messages for the pipeline in view",
        ));
        expander.add_row(&captured);
        expander.add_row(&processed);
        expander.add_row(&underruns);
        expander.add_row(&overruns);
        expander.add_row(&trims);
        expander.add_row(&occupancy);
        expander.add_row(&warnings);

        Self {
            expander,
            captured,
            processed,
            underruns,
            overruns,
            trims,
            occupancy,
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
        self.overruns
            .set_subtitle(&stats.overrun_samples.to_string());
        self.trims.set_subtitle(&stats.trim_events.to_string());
        self.occupancy
            .set_subtitle(&stats.ring_occupancy.to_string());
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
