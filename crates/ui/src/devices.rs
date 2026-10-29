use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use pwe_core::PipelineKind;
use pwe_core::i18n::{t, tr};
use pwe_engine::{EngineState, Snapshot, StatsSnapshot};

use crate::format;
use crate::pages::WorkspaceActions;

pub struct DevicesView {
    page: adw::PreferencesPage,
    state_row: adw::ActionRow,
    rate_row: adw::ActionRow,
    node_row: adw::ActionRow,
    routing_row: adw::ActionRow,
    reconnect: gtk::Button,
    devices: adw::ComboRow,
    device_names: RefCell<Vec<String>>,
    updating: Rc<Cell<bool>>,
    microphone: adw::SwitchRow,
    destination: RefCell<String>,
    status: RefCell<String>,
    toggles: Rc<Cell<bool>>,
}

impl DevicesView {
    pub fn new(actions: WorkspaceActions) -> (adw::PreferencesPage, Self) {
        let page = adw::PreferencesPage::new();

        let engine = adw::PreferencesGroup::new();
        engine.set_title(t("Engine"));

        let state_row = adw::ActionRow::new();
        state_row.set_title(t("Status"));
        state_row.set_subtitle(t("Connecting"));
        let reconnect = gtk::Button::with_label(t("Reconnect"));
        reconnect.add_css_class("flat");
        reconnect.set_valign(gtk::Align::Center);
        reconnect.set_visible(false);
        reconnect.set_tooltip_text(Some(t("Rebuild the streams against the audio server")));
        {
            let on_reconnect = Rc::clone(&actions.on_reconnect);
            reconnect.connect_clicked(move |_| on_reconnect());
        }
        state_row.add_suffix(&reconnect);

        let rate_row = adw::ActionRow::new();
        rate_row.set_title(t("Format"));
        rate_row.set_subtitle(t("unknown"));

        let node_row = adw::ActionRow::new();
        node_row.set_title(t("Virtual sink"));
        node_row.set_subtitle("pipewire-effects-sink");
        let copy = gtk::Button::from_icon_name(format::ICON_COPY);
        copy.add_css_class("flat");
        copy.set_valign(gtk::Align::Center);
        copy.set_tooltip_text(Some(t("Copy the node name")));
        {
            let node = node_row.clone();
            copy.connect_clicked(move |button| {
                let name = node.widget_name().to_string();
                button.display().clipboard().set_text(&name);
                button.set_icon_name(format::ICON_COPIED);
                let button = button.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                    button.set_icon_name(format::ICON_COPY);
                });
            });
        }
        node_row.add_suffix(&copy);

        let routing_row = adw::ActionRow::new();
        routing_row.set_title(t("Routing"));
        routing_row.set_subtitle_lines(3);

        engine.add(&state_row);
        engine.add(&rate_row);
        engine.add(&node_row);
        engine.add(&routing_row);
        page.add(&engine);

        let output = adw::PreferencesGroup::new();
        output.set_title(t("Destination"));
        let devices = adw::ComboRow::new();
        devices.set_title(t("Output device"));
        devices.set_subtitle(t("The hardware device that receives the effect chain"));
        let updating = Rc::new(Cell::new(false));
        let flag = Rc::clone(&updating);
        let names = Rc::new(RefCell::new(Vec::<String>::new()));
        let names_clone = Rc::clone(&names);
        let on_target = Rc::clone(&actions.on_target);
        devices.connect_selected_notify(move |row| {
            if flag.get() {
                return;
            }
            let index = row.selected() as usize;
            if let Some(name) = names_clone.borrow().get(index) {
                on_target(name.clone());
            }
        });
        output.add(&devices);
        page.add(&output);

        let monitoring = adw::PreferencesGroup::new();
        monitoring.set_title(t("Microphone"));
        let toggles = Rc::new(Cell::new(false));
        let microphone = adw::SwitchRow::new();
        microphone.set_title(t("Process microphone input"));
        microphone.set_subtitle(t(
            "Runs the input chain and publishes it as the PipeWire Effects Source",
        ));
        {
            let flag = Rc::clone(&toggles);
            let on_input = Rc::clone(&actions.on_input);
            microphone.connect_active_notify(move |row| {
                if flag.get() {
                    return;
                }
                on_input(row.is_active());
            });
        }
        monitoring.add(&microphone);
        page.add(&monitoring);

        let view = Self {
            page: page.clone(),
            state_row,
            rate_row,
            node_row,
            routing_row,
            reconnect,
            devices,
            device_names: RefCell::new(Vec::new()),
            updating,
            microphone,
            destination: RefCell::new(String::new()),
            status: RefCell::new(String::new()),
            toggles,
        };
        (page, view)
    }

    pub fn page(&self) -> &adw::PreferencesPage {
        &self.page
    }

    pub fn set_running(&self, running: bool) {
        self.toggles.set(true);
        self.microphone.set_active(running);
        self.toggles.set(false);
    }

    pub fn update(&self, snapshot: &Snapshot, stats: &StatsSnapshot, state: EngineState) {
        let kind = snapshot.kind;
        let status = tr("{0} · {1} pipeline", &[state.label(), kind.label()]);
        if *self.status.borrow() != status {
            self.status_row(&status, state == EngineState::Failed);
            *self.status.borrow_mut() = status;
        }
        self.reconnect.set_visible(state == EngineState::Failed);
        self.rate_row.set_subtitle(&if snapshot.sample_rate == 0 {
            t("waiting for the graph").to_owned()
        } else {
            tr(
                "{0} Hz · {1} channels · {2} frames per buffer",
                &[
                    &snapshot.sample_rate.to_string(),
                    &snapshot.channels.to_string(),
                    &stats.buffer_frames.to_string(),
                ],
            )
        });
        self.node_row.set_title(match kind {
            PipelineKind::Output => t("Virtual sink"),
            PipelineKind::Input => t("Virtual source"),
        });
        let node = if snapshot.node_name.is_empty() {
            pwe_engine::virtual_node_name(kind).to_owned()
        } else {
            snapshot.node_name.clone()
        };
        self.node_row.set_subtitle(&node);
        self.node_row.set_widget_name(&node);
        self.routing_row.set_subtitle(match kind {
            PipelineKind::Output => t(
                "Pick PipeWire Effects Sink as the output in an application, or make it the default sink.",
            ),
            PipelineKind::Input => t(
                "Pick PipeWire Effects Source as the input in an application, or make it the default source.",
            ),
        });
        self.sync_devices(snapshot);
        self.sync_destination(snapshot);
    }

    fn status_row(&self, status: &str, failed: bool) {
        self.state_row.remove_css_class("error");
        self.state_row.set_subtitle(status);
        if failed {
            self.state_row.add_css_class("error");
        }
    }

    fn sync_destination(&self, snapshot: &Snapshot) {
        let described = snapshot
            .devices
            .iter()
            .find(|device| device.name == snapshot.target)
            .map_or_else(
                || snapshot.target.clone(),
                |device| {
                    if device.description.is_empty() {
                        device.name.clone()
                    } else {
                        device.description.clone()
                    }
                },
            );
        let noun = snapshot.kind.noun();
        let subtitle = if snapshot.target.is_empty() {
            tr("No {0} device is available", &[noun])
        } else if snapshot.automatic {
            tr("Following the system default, {0}", &[&described])
        } else {
            described
        };
        if *self.destination.borrow() == subtitle {
            return;
        }
        self.devices.set_title(match snapshot.kind {
            PipelineKind::Output => t("Output device"),
            PipelineKind::Input => t("Input device"),
        });
        self.devices.set_subtitle(&subtitle);
        *self.destination.borrow_mut() = subtitle;
    }

    fn sync_devices(&self, snapshot: &Snapshot) {
        let own = snapshot.node_name.as_str();
        let matching: Vec<(String, String)> = snapshot
            .devices
            .iter()
            .filter(|device| device.name != own)
            .filter(|device| match snapshot.kind {
                PipelineKind::Output => device.is_sink(),
                PipelineKind::Input => device.is_source(),
            })
            .map(|device| (device.name.clone(), device.description.clone()))
            .collect();
        let mut names = vec![pwe_engine::AUTO_TARGET.to_owned()];
        names.extend(matching.iter().map(|(name, _)| name.clone()));
        if *self.device_names.borrow() == names {
            return;
        }
        let mut labels = vec![t("System default").to_owned()];
        labels.extend(device_labels(&matching));
        let selected = if snapshot.automatic {
            0
        } else {
            names
                .iter()
                .position(|name| *name == snapshot.target)
                .unwrap_or(0)
        };
        self.updating.set(true);
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        self.devices.set_model(Some(&gtk::StringList::new(&refs)));
        self.devices.set_selected(selected as u32);
        self.updating.set(false);
        *self.device_names.borrow_mut() = names;
    }
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
