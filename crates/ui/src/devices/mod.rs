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
    pipeline_group: adw::PreferencesGroup,
    format_row: adw::ActionRow,
    quantum_row: adw::ActionRow,
    reconnect: gtk::Button,
    devices: adw::ComboRow,
    device_names: RefCell<Vec<String>>,
    updating: Rc<Cell<bool>>,
    node_row: adw::ActionRow,
    hint: gtk::Label,
    microphone: adw::SwitchRow,
    source_row: adw::ActionRow,
    destination: RefCell<String>,
    toggles: Rc<Cell<bool>>,
}

impl DevicesView {
    pub fn new(actions: WorkspaceActions) -> (adw::PreferencesPage, Self) {
        let page = adw::PreferencesPage::new();

        let pipeline_group = adw::PreferencesGroup::new();
        pipeline_group.set_title(t("Pipeline"));

        let format_row = adw::ActionRow::new();
        format_row.set_title(t("Format"));
        format_row.set_subtitle(t("Waiting for the graph"));
        let reconnect = gtk::Button::with_label(t("Reconnect"));
        reconnect.add_css_class("flat");
        reconnect.set_valign(gtk::Align::Center);
        reconnect.set_visible(false);
        reconnect.set_tooltip_text(Some(t("Rebuild the streams against the audio server")));
        {
            let on_reconnect = Rc::clone(&actions.on_reconnect);
            reconnect.connect_clicked(move |_| on_reconnect());
        }
        format_row.add_suffix(&reconnect);

        let quantum_row = adw::ActionRow::new();
        quantum_row.set_title(t("Quantum"));
        quantum_row.set_subtitle("—");

        pipeline_group.add(&format_row);
        pipeline_group.add(&quantum_row);
        page.add(&pipeline_group);

        let routing = adw::PreferencesGroup::new();
        routing.set_title(t("Routing"));

        let devices = adw::ComboRow::new();
        devices.set_title(t("Output device"));
        devices.set_subtitle(t("Automatic"));
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
        routing.add(&devices);

        let node_row = copyable_row(t("Node name"));
        routing.add(&node_row);

        let hint = gtk::Label::new(None);
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        hint.set_margin_start(12);
        hint.set_margin_end(12);
        hint.set_margin_top(6);
        hint.set_margin_bottom(6);
        routing.add(&hint);
        page.add(&routing);

        let monitoring = adw::PreferencesGroup::new();
        monitoring.set_title(t("Microphone input"));
        let toggles = Rc::new(Cell::new(false));
        let microphone = adw::SwitchRow::new();
        microphone.set_title(t("Process the microphone"));
        microphone.set_subtitle(t(
            "Runs the input chain and publishes the result as a virtual source",
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
        let source_row = copyable_row(t("Virtual source"));
        source_row.set_subtitle("pipewire-effects-source");
        source_row.set_widget_name("pipewire-effects-source");
        monitoring.add(&source_row);
        page.add(&monitoring);

        let view = Self {
            pipeline_group,
            format_row,
            quantum_row,
            reconnect,
            devices,
            device_names: RefCell::new(Vec::new()),
            updating,
            node_row,
            hint,
            microphone,
            source_row,
            destination: RefCell::new(String::new()),
            toggles,
        };
        (page, view)
    }

    pub fn set_running(&self, running: bool) {
        self.toggles.set(true);
        self.microphone.set_active(running);
        self.source_row.set_visible(running);
        self.toggles.set(false);
    }

    pub fn update(&self, snapshot: &Snapshot, stats: &StatsSnapshot, state: EngineState) {
        let kind = snapshot.kind;
        self.pipeline_group
            .set_title(&tr("{0} pipeline", &[kind.label()]));
        self.reconnect.set_visible(state == EngineState::Failed);
        self.format_row.set_subtitle(&if snapshot.sample_rate == 0 {
            t("Waiting for the graph").to_owned()
        } else {
            tr(
                "{0} · {1} channels",
                &[
                    &format::rate_text(snapshot.sample_rate),
                    &snapshot.channels.to_string(),
                ],
            )
        });
        self.quantum_row.set_subtitle(&if stats.buffer_frames == 0 {
            "—".to_owned()
        } else {
            tr(
                "{0} frames · {1}",
                &[
                    &format::grouped(stats.buffer_frames),
                    &format::duration_text(stats.buffer_frames, snapshot.sample_rate),
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
        self.hint.set_label(match kind {
            PipelineKind::Output => {
                t("Route an application to this sink, or make it the default output.")
            }
            PipelineKind::Input => {
                t("Record from this source in an application, or make it the default input.")
            }
        });
        self.sync_devices(snapshot);
        self.sync_destination(snapshot);
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
            tr("System default · {0}", &[&described])
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

fn copyable_row(title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    let copy = gtk::Button::from_icon_name(format::ICON_COPY);
    copy.add_css_class("flat");
    copy.set_valign(gtk::Align::Center);
    copy.set_tooltip_text(Some(t("Copy this name to the clipboard")));
    copy.update_property(&[gtk::accessible::Property::Label(t(
        "Copy this name to the clipboard",
    ))]);
    let source = row.clone();
    copy.connect_clicked(move |button| {
        let name = source.widget_name().to_string();
        if name.is_empty() {
            return;
        }
        button.display().clipboard().set_text(&name);
        button.set_icon_name(format::ICON_COPIED);
        let button = button.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
            button.set_icon_name(format::ICON_COPY);
        });
    });
    row.add_suffix(&copy);
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
#[path = "tests/tests.rs"]
mod tests;
