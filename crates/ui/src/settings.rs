use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use pwe_engine::{EngineState, Snapshot, StatsSnapshot};

pub struct SettingsView {
    page: adw::PreferencesPage,
    state_row: adw::ActionRow,
    rate_row: adw::ActionRow,
    sink_row: adw::ActionRow,
    devices: adw::ComboRow,
    device_names: RefCell<Vec<String>>,
    updating: Rc<Cell<bool>>,
    captured: adw::ActionRow,
    processed: adw::ActionRow,
    underruns: adw::ActionRow,
    trims: adw::ActionRow,
    warnings: adw::ActionRow,
}

impl SettingsView {
    pub fn new(on_target: Rc<dyn Fn(String)>) -> (adw::PreferencesPage, Self) {
        let page = adw::PreferencesPage::new();

        let engine = adw::PreferencesGroup::new();
        engine.set_title("Engine");
        let state_row = adw::ActionRow::new();
        state_row.set_title("Status");
        state_row.set_subtitle("Connecting");
        let rate_row = adw::ActionRow::new();
        rate_row.set_title("Sample rate");
        rate_row.set_subtitle("unknown");
        let sink_row = adw::ActionRow::new();
        sink_row.set_title("Virtual sink");
        sink_row.set_subtitle("pipewire-effects-sink");
        let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
        copy.set_tooltip_text(Some("Copy the node name"));
        copy.connect_clicked(|button| {
            button
                .display()
                .clipboard()
                .set_text("pipewire-effects-sink");
        });
        sink_row.add_suffix(&copy);
        engine.add(&state_row);
        engine.add(&rate_row);
        engine.add(&sink_row);
        page.add(&engine);

        let output = adw::PreferencesGroup::new();
        output.set_title("Output device");
        let devices = adw::ComboRow::new();
        devices.set_title("Destination");
        devices.set_subtitle("The hardware device the processed audio is played on");
        let target = on_target.clone();
        let updating = Rc::new(Cell::new(false));
        let flag = Rc::clone(&updating);
        let names = Rc::new(RefCell::new(Vec::<String>::new()));
        let names_clone = Rc::clone(&names);
        devices.connect_selected_notify(move |row| {
            if flag.get() {
                return;
            }
            let index = row.selected() as usize;
            if let Some(name) = names_clone.borrow().get(index) {
                target(name.clone());
            }
        });
        output.add(&devices);
        page.add(&output);

        let stats = adw::PreferencesGroup::new();
        stats.set_title("Statistics");
        let captured = counter_row("Captured frames");
        let processed = counter_row("Processed frames");
        let underruns = counter_row("Underrun samples");
        let trims = counter_row("Drift trims");
        stats.add(&captured);
        stats.add(&processed);
        stats.add(&underruns);
        stats.add(&trims);
        page.add(&stats);

        let diagnostics = adw::PreferencesGroup::new();
        diagnostics.set_title("Diagnostics");
        let warnings = adw::ActionRow::new();
        warnings.set_title("Messages");
        warnings.set_subtitle("None");
        diagnostics.add(&warnings);
        page.add(&diagnostics);

        let view = Self {
            page: page.clone(),
            state_row,
            rate_row,
            sink_row,
            devices,
            device_names: RefCell::new(Vec::new()),
            updating,
            captured,
            processed,
            underruns,
            trims,
            warnings,
        };
        (page, view)
    }

    pub fn page(&self) -> &adw::PreferencesPage {
        &self.page
    }

    pub fn update(&self, snapshot: &Snapshot, stats: &StatsSnapshot, state: EngineState) {
        self.state_row.set_subtitle(state.label());
        self.rate_row.set_subtitle(&if snapshot.sample_rate == 0 {
            "waiting for the graph".to_owned()
        } else {
            format!(
                "{} Hz · {} channels · {} frames per buffer",
                snapshot.sample_rate, snapshot.channels, stats.buffer_frames
            )
        });
        self.sink_row.set_subtitle(&format!(
            "{} - route applications here",
            if snapshot.sink_node_name.is_empty() {
                "pipewire-effects-sink"
            } else {
                &snapshot.sink_node_name
            }
        ));
        self.captured
            .set_subtitle(&stats.captured_frames.to_string());
        self.processed
            .set_subtitle(&stats.processed_frames.to_string());
        self.underruns
            .set_subtitle(&stats.underrun_samples.to_string());
        self.trims.set_subtitle(&stats.trim_events.to_string());
        self.warnings
            .set_subtitle(&if snapshot.warnings.is_empty() {
                "None".to_owned()
            } else {
                snapshot.warnings.join("\n")
            });
        self.sync_devices(snapshot);
    }

    fn sync_devices(&self, snapshot: &Snapshot) {
        let sinks: Vec<String> = snapshot
            .devices
            .iter()
            .filter(|device| device.is_sink() && device.name != snapshot.sink_node_name)
            .map(|device| device.name.clone())
            .collect();
        if sinks.is_empty() || *self.device_names.borrow() == sinks {
            return;
        }
        self.updating.set(true);
        let labels: Vec<&str> = sinks.iter().map(String::as_str).collect();
        self.devices.set_model(Some(&gtk::StringList::new(&labels)));
        let selected = sinks
            .iter()
            .position(|name| *name == snapshot.target)
            .unwrap_or(0);
        self.devices.set_selected(selected as u32);
        self.updating.set(false);
        *self.device_names.borrow_mut() = sinks;
    }
}

fn counter_row(title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle("0");
    row
}
