use std::cell::RefCell;

use adw::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::{EngineState, Snapshot, StatsSnapshot};

use crate::format;

pub struct Diagnostics {
    page: adw::PreferencesPage,
    expander: adw::ExpanderRow,
    pipeline_row: adw::ActionRow,
    rate_row: adw::ActionRow,
    quantum_row: adw::ActionRow,
    occupancy_row: adw::ActionRow,
    captured_row: adw::ActionRow,
    processed_row: adw::ActionRow,
    underrun_row: adw::ActionRow,
    overrun_row: adw::ActionRow,
    trim_row: adw::ActionRow,
    messages_group: adw::PreferencesGroup,
    empty_row: adw::ActionRow,
    message_rows: RefCell<Vec<adw::ActionRow>>,
    last_messages: RefCell<Vec<String>>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnostics {
    pub fn new() -> Self {
        let pipeline_row = value_row(t("Pipeline"));
        let rate_row = value_row(t("Sample rate"));
        let quantum_row = value_row(t("Quantum"));
        let occupancy_row = value_row(t("Ring occupancy"));
        let captured_row = value_row(t("Captured frames"));
        let processed_row = value_row(t("Processed frames"));
        let underrun_row = value_row(t("Underruns"));
        let overrun_row = value_row(t("Overruns"));
        let trim_row = value_row(t("Drift trims"));

        let expander = adw::ExpanderRow::new();
        expander.set_title(t("Engine counters"));
        expander.set_subtitle(t("Live values for the pipeline in view"));
        for row in [
            &pipeline_row,
            &rate_row,
            &quantum_row,
            &occupancy_row,
            &captured_row,
            &processed_row,
            &underrun_row,
            &overrun_row,
            &trim_row,
        ] {
            expander.add_row(row);
        }

        let counters = adw::PreferencesGroup::new();
        counters.set_title(t("Engine"));
        counters.add(&expander);

        let messages_group = adw::PreferencesGroup::new();
        messages_group.set_title(t("Messages"));
        let empty_row = adw::ActionRow::new();
        empty_row.set_title(t("No messages"));
        empty_row.add_css_class("dim-label");
        messages_group.add(&empty_row);

        let page = adw::PreferencesPage::new();
        page.add(&counters);
        page.add(&messages_group);

        Self {
            page,
            expander,
            pipeline_row,
            rate_row,
            quantum_row,
            occupancy_row,
            captured_row,
            processed_row,
            underrun_row,
            overrun_row,
            trim_row,
            messages_group,
            empty_row,
            message_rows: RefCell::new(Vec::new()),
            last_messages: RefCell::new(Vec::new()),
        }
    }

    pub fn page(&self) -> &adw::PreferencesPage {
        &self.page
    }

    pub fn expand(&self) {
        self.expander.set_expanded(true);
    }

    pub fn update(
        &self,
        snapshot: &Snapshot,
        _state: EngineState,
        stats: &StatsSnapshot,
        warnings: &[String],
    ) {
        self.pipeline_row.set_subtitle(snapshot.kind.label());
        self.rate_row.set_subtitle(&if snapshot.sample_rate == 0 {
            "—".to_owned()
        } else {
            format::rate_text(snapshot.sample_rate)
        });
        self.quantum_row.set_subtitle(&if stats.buffer_frames == 0 {
            "—".to_owned()
        } else {
            format!(
                "{} · {}",
                t_frames(stats.buffer_frames),
                format::duration_text(stats.buffer_frames, snapshot.sample_rate)
            )
        });
        self.occupancy_row
            .set_subtitle(&t_samples(stats.ring_occupancy));
        self.captured_row
            .set_subtitle(&t_frames(stats.captured_frames));
        self.processed_row
            .set_subtitle(&t_frames(stats.processed_frames));
        self.underrun_row
            .set_subtitle(&t_samples(stats.underrun_samples));
        self.overrun_row
            .set_subtitle(&t_samples(stats.overrun_samples));
        self.trim_row
            .set_subtitle(&format::grouped(stats.trim_events));
        self.sync_messages(warnings);
    }

    fn sync_messages(&self, warnings: &[String]) {
        if *self.last_messages.borrow() == warnings {
            return;
        }
        for row in self.message_rows.borrow().iter() {
            self.messages_group.remove(row);
        }
        self.message_rows.borrow_mut().clear();
        self.empty_row.set_visible(warnings.is_empty());
        for warning in warnings {
            let row = adw::ActionRow::new();
            row.set_title(warning);
            row.set_title_lines(3);
            self.messages_group.add(&row);
            self.message_rows.borrow_mut().push(row);
        }
        *self.last_messages.borrow_mut() = warnings.to_vec();
    }
}

fn value_row(title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle("—");
    row.add_css_class("numeric");
    row
}

fn t_frames(frames: u64) -> String {
    format!("{} frames", format::grouped(frames))
}

fn t_samples(samples: u64) -> String {
    format!("{} samples", format::grouped(samples))
}
