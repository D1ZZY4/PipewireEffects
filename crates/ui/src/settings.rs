use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
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
        rate_row.set_title("Format");
        rate_row.set_subtitle("unknown");

        let sink_row = adw::ActionRow::new();
        sink_row.set_title("Virtual sink");
        sink_row.set_subtitle("pipewire-effects-sink");
        let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
        copy.add_css_class("flat");
        copy.set_valign(gtk::Align::Center);
        copy.set_tooltip_text(Some("Copy the node name"));
        copy.connect_clicked(|button| {
            button
                .display()
                .clipboard()
                .set_text("pipewire-effects-sink");
            button.set_icon_name("object-select-symbolic");
            let button = button.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                button.set_icon_name("edit-copy-symbolic");
            });
        });
        sink_row.add_suffix(&copy);

        let routing_row = adw::ActionRow::new();
        routing_row.set_title("Routing");
        routing_row.set_subtitle(
            "Pick PipeWire Effects Sink as the output in an application, or make it the default sink.",
        );
        routing_row.set_subtitle_lines(2);

        engine.add(&state_row);
        engine.add(&rate_row);
        engine.add(&sink_row);
        engine.add(&routing_row);
        page.add(&engine);

        let output = adw::PreferencesGroup::new();
        output.set_title("Destination");
        let devices = adw::ComboRow::new();
        devices.set_title("Play processed audio on");
        devices.set_subtitle("The hardware device that receives the effect chain");
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

        let advanced = adw::PreferencesGroup::new();
        advanced.set_title("Advanced");

        let captured = counter_row("Captured frames");
        let processed = counter_row("Processed frames");
        let underruns = counter_row("Underrun samples");
        let trims = counter_row("Drift trims");
        let warnings = adw::ActionRow::new();
        warnings.set_title("Messages");
        warnings.set_subtitle("None");
        warnings.set_subtitle_lines(4);

        let expander = adw::ExpanderRow::new();
        expander.set_title("Diagnostics");
        expander.set_subtitle("Audio counters and engine messages");
        expander.add_row(&captured);
        expander.add_row(&processed);
        expander.add_row(&underruns);
        expander.add_row(&trims);
        expander.add_row(&warnings);
        advanced.add(&expander);
        page.add(&advanced);

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
        self.state_row.remove_css_class("error");
        if state == EngineState::Failed {
            self.state_row.add_css_class("error");
        }
        self.rate_row.set_subtitle(&if snapshot.sample_rate == 0 {
            "waiting for the graph".to_owned()
        } else {
            format!(
                "{} Hz · {} channels · {} frames per buffer",
                snapshot.sample_rate, snapshot.channels, stats.buffer_frames
            )
        });
        self.sink_row
            .set_subtitle(&if snapshot.sink_node_name.is_empty() {
                "pipewire-effects-sink".to_owned()
            } else {
                snapshot.sink_node_name.clone()
            });
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
        let sinks: Vec<(String, String)> = snapshot
            .devices
            .iter()
            .filter(|device| device.is_sink() && device.name != snapshot.sink_node_name)
            .map(|device| (device.name.clone(), device.description.clone()))
            .collect();
        if sinks.is_empty()
            || *self.device_names.borrow()
                == sinks
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>()
        {
            return;
        }
        let labels = device_labels(&sinks);
        let selected = sinks
            .iter()
            .position(|(name, _)| *name == snapshot.target)
            .unwrap_or(0);
        self.updating.set(true);
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        self.devices.set_model(Some(&gtk::StringList::new(&refs)));
        self.devices.set_selected(selected as u32);
        self.updating.set(false);
        *self.device_names.borrow_mut() = sinks.into_iter().map(|(name, _)| name).collect();
    }
}

fn device_labels(sinks: &[(String, String)]) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    sinks
        .iter()
        .map(|(name, description)| {
            let description = if description.is_empty() {
                name.as_str()
            } else {
                description.as_str()
            };
            let repeated = sinks
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

fn counter_row(title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle("0");
    row
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
