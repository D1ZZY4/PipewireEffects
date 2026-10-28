//! The main window.
//!
//! A sidebar and a detail area, in the shape GNOME applications use: the
//! sidebar answers "what is in the chain and is it working", the detail area
//! answers "what does this effect do".
//!
//! # How state flows
//!
//! Widgets never mutate anything. They emit a [`UiAction`]; the window's action
//! handler performs the mutation through the domain objects, and the affected
//! views are rebuilt. The periodic tick is deliberately separate and only ever
//! *reads*: meters, counters and engine events. Keeping writes on the action
//! path and reads on the tick path is what stops a redraw from racing an edit.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{Orientation, gio, glib};
use pipewire_effects_core::{ChainRecipe, MeterSnapshot};
use pipewire_effects_engine::{AudioDevice, ConnectionState, EngineEvent, StreamRole, list_sinks};
use pipewire_effects_presets::{Preset, PresetInfo};

use crate::action::{Dispatch, UiAction};
use crate::chain::{self, Rows};
use crate::editor;
use crate::runtime::Runtime;

/// How often the interface refreshes meters and drains engine events.
///
/// Twenty times a second is smooth for a meter and cheap enough to be
/// irrelevant next to audio processing.
const TICK: Duration = Duration::from_millis(50);

/// How far a meter falls back per tick, as a fraction of full scale.
const METER_DECAY: f32 = 0.015;

/// The lowest level a meter shows, in dBFS.
const METER_FLOOR_DB: f32 = -60.0;

/// The values that change while the application runs.
struct State {
    runtime: Rc<Runtime>,
    window: glib::WeakRef<adw::ApplicationWindow>,
    toast: adw::ToastOverlay,
    content: gtk::Box,
    dispatch: RefCell<Option<Dispatch>>,

    chain_group: adw::PreferencesGroup,
    chain_rows: Rows,
    preset_group: adw::PreferencesGroup,
    preset_rows: Rows,
    device_group: adw::PreferencesGroup,
    device_rows: Rows,

    engine_row: adw::ActionRow,
    node_row: adw::ActionRow,
    format_row: adw::ActionRow,
    latency_row: adw::ActionRow,
    glitch_row: adw::ActionRow,

    input_bars: Vec<gtk::LevelBar>,
    output_bars: Vec<gtk::LevelBar>,
    input_readouts: Vec<gtk::Label>,
    output_readouts: Vec<gtk::Label>,
    input_hold: RefCell<Vec<f32>>,
    output_hold: RefCell<Vec<f32>>,

    selected: Cell<usize>,
    devices: RefCell<Vec<AudioDevice>>,
    presets: RefCell<Vec<PresetInfo>>,
    closing: Cell<bool>,
}

/// Builds the application window.
pub fn build(runtime: Rc<Runtime>, app: &adw::Application) -> adw::ApplicationWindow {
    let channels = runtime.config().format.channels_usize().max(1);

    // ---- Sidebar: the chain, presets, devices, meters and status ---------
    let chain_group = adw::PreferencesGroup::builder()
        .title("Chain")
        .description("Audio flows through these effects from top to bottom.")
        .build();
    let preset_group = adw::PreferencesGroup::builder()
        .title("Presets")
        .description("Whole chains, stored as JSON files you can share.")
        .build();
    let device_group = adw::PreferencesGroup::builder()
        .title("Output")
        .description("Where the processed audio is played.")
        .build();

    let mut input_bars = Vec::with_capacity(channels);
    let mut output_bars = Vec::with_capacity(channels);
    let mut input_readouts = Vec::with_capacity(channels);
    let mut output_readouts = Vec::with_capacity(channels);
    let levels_group = adw::PreferencesGroup::builder().title("Levels").build();
    for channel in 0..channels {
        let name = if channels == 2 {
            if channel == 0 { "Left" } else { "Right" }.to_owned()
        } else {
            format!("Channel {}", channel + 1)
        };

        let (input_row, input_bar, input_readout) = meter_row(&name, "Before processing");
        let (output_row, output_bar, output_readout) = meter_row(&name, "After processing");
        levels_group.add(&input_row);
        levels_group.add(&output_row);

        input_bars.push(input_bar);
        input_readouts.push(input_readout);
        output_bars.push(output_bar);
        output_readouts.push(output_readout);
    }

    let engine_row = status_row("Engine", "Starting…");
    let node_row = status_row("Virtual sink", "—");
    let format_row = status_row("Format", "—");
    let latency_row = status_row("Buffer", "—");
    let glitch_row = status_row("Dropped audio", "None");
    let status_group = adw::PreferencesGroup::builder().title("Engine").build();
    for row in [
        &engine_row,
        &node_row,
        &format_row,
        &latency_row,
        &glitch_row,
    ] {
        status_group.add(row);
    }

    let sidebar_page = adw::PreferencesPage::new();
    sidebar_page.add(&device_group);
    sidebar_page.add(&chain_group);
    sidebar_page.add(&preset_group);
    sidebar_page.add(&levels_group);
    sidebar_page.add(&status_group);

    let sidebar_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&sidebar_page)
        .build();
    let sidebar = adw::NavigationPage::new(&sidebar_scroller, "PipewireEffects");

    // ---- Detail: the selected effect ------------------------------------
    let content = gtk::Box::new(Orientation::Vertical, 0);
    content.append(&editor::build_empty_page());
    let content_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();
    let detail = adw::NavigationPage::new(&content_scroller, "Effect");

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&sidebar));
    split.set_content(Some(&detail));

    // ---- Header ----------------------------------------------------------
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "PipewireEffects",
        "Audio effects for PipeWire",
    )));

    let bypass = gtk::Switch::builder()
        .valign(gtk::Align::Center)
        .tooltip_text("Pass audio through untouched")
        .build();
    let bypass_box = gtk::Box::new(Orientation::Horizontal, 6);
    bypass_box.append(&gtk::Label::new(Some("Bypass")));
    bypass_box.append(&bypass);
    header.pack_end(&bypass_box);

    let menu = gio::Menu::new();
    menu.append(Some("Refresh devices"), Some("win.refresh-devices"));
    menu.append(Some("Restart engine"), Some("win.restart-engine"));
    menu.append(Some("About PipewireEffects"), Some("win.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .build();
    header.pack_end(&menu_button);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&split));

    let toast = adw::ToastOverlay::new();
    toast.set_child(Some(&toolbar));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("PipewireEffects")
        .content(&toast)
        .default_width(1_020)
        .default_height(720)
        .build();

    // ---- State -----------------------------------------------------------
    let state = Rc::new(State {
        runtime,
        window: window.downgrade(),
        toast,
        content,
        dispatch: RefCell::new(None),
        chain_group,
        chain_rows: Rc::new(RefCell::new(Vec::new())),
        preset_group,
        preset_rows: Rc::new(RefCell::new(Vec::new())),
        device_group,
        device_rows: Rc::new(RefCell::new(Vec::new())),
        engine_row,
        node_row,
        format_row,
        latency_row,
        glitch_row,
        input_bars,
        output_bars,
        input_readouts,
        output_readouts,
        input_hold: RefCell::new(vec![0.0; channels]),
        output_hold: RefCell::new(vec![0.0; channels]),
        selected: Cell::new(0),
        devices: RefCell::new(Vec::new()),
        presets: RefCell::new(Vec::new()),
        closing: Cell::new(false),
    });

    // The dispatch holds only a weak reference to the state, so the widgets
    // that store it do not keep the window alive.
    let weak_state = Rc::downgrade(&state);
    let dispatch: Dispatch = Rc::new(move |action| {
        if let Some(state) = weak_state.upgrade() {
            state.handle(action);
        }
    });
    *state.dispatch.borrow_mut() = Some(Rc::clone(&dispatch));

    // ---- Signals and actions --------------------------------------------
    {
        let dispatch = Rc::clone(&dispatch);
        bypass.connect_state_set(move |_, active| {
            dispatch(UiAction::SetBypassed(active));
            glib::Propagation::Proceed
        });
    }

    add_action(&window, "refresh-devices", {
        let dispatch = Rc::clone(&dispatch);
        move |_, _| dispatch(UiAction::RefreshDevices)
    });
    add_action(&window, "restart-engine", {
        let dispatch = Rc::clone(&dispatch);
        move |_, _| dispatch(UiAction::RestartEngine)
    });
    {
        let weak_window = window.downgrade();
        add_action(&window, "about", move |_, _| {
            let Some(window) = weak_window.upgrade() else {
                return;
            };
            adw::AboutDialog::builder()
                .application_name("PipewireEffects")
                .application_icon(crate::APPLICATION_ID)
                .version(env!("CARGO_PKG_VERSION"))
                .comments(
                    "A fast, modern PipeWire audio effects processor: an equaliser, \
                     a limiter and a gain stage you wire together yourself.",
                )
                .website("https://github.com/D1ZZY4/PipewireEffects")
                .license_type(gtk::License::Gpl30)
                .build()
                .present(Some(&window));
        });
    }

    {
        let closing = state.closing.clone();
        window.connect_close_request(move |_| {
            closing.set(true);
            glib::Propagation::Proceed
        });
    }

    state.refresh_devices();
    state.refresh_presets();
    state.refresh_chain();
    state.refresh_content();
    state.refresh_status();

    // ---- The periodic read-only tick ------------------------------------
    {
        let state = Rc::clone(&state);
        glib::timeout_add_local(TICK, move || {
            state.drain_events();
            state.update_meters();
            state.refresh_status();
            if state.closing.get() {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    window
}

/// Adds an action to the window, so a menu item named `win.<name>` can reach it.
fn add_action<F>(window: &adw::ApplicationWindow, name: &str, handler: F)
where
    F: Fn(&gio::SimpleAction, Option<&glib::Variant>) + 'static,
{
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(handler);
    window.add_action(&action);
}

/// A status line in the engine group.
fn status_row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build()
}

/// A label plus a level bar, in a row.
fn meter_row(name: &str, role: &str) -> (adw::ActionRow, gtk::LevelBar, gtk::Label) {
    let row = adw::ActionRow::builder().title(name).subtitle(role).build();

    let bar = gtk::LevelBar::builder()
        .min_value(0.0)
        .max_value(1.0)
        .value(0.0)
        .width_request(120)
        .valign(gtk::Align::Center)
        .build();

    let readout = gtk::Label::new(Some("—"));
    readout.add_css_class("dim-label");
    readout.add_css_class("numeric");
    readout.set_width_chars(8);
    readout.set_xalign(1.0);
    readout.set_valign(gtk::Align::Center);

    row.add_suffix(&bar);
    row.add_suffix(&readout);
    (row, bar, readout)
}

impl State {
    fn dispatch(&self) -> Dispatch {
        self.dispatch
            .borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(|_| {}))
    }

    /// Applies a user action.
    fn handle(self: &Rc<Self>, action: UiAction) {
        match action {
            UiAction::Select(index) => {
                self.selected.set(index);
                self.refresh_content();
            }
            UiAction::SetEnabled(index, enabled) => {
                self.report(self.runtime.controller().set_enabled(index, enabled));
                self.refresh_chain();
            }
            UiAction::MoveUp(index) => {
                if index > 0 {
                    self.report(self.runtime.controller().move_entry(index, index - 1));
                    self.selected.set(index - 1);
                    self.refresh_chain();
                    self.refresh_content();
                }
            }
            UiAction::MoveDown(index) => {
                let last = self.runtime.controller().len().saturating_sub(1);
                if index < last {
                    self.report(self.runtime.controller().move_entry(index, index + 1));
                    self.selected.set(index + 1);
                    self.refresh_chain();
                    self.refresh_content();
                }
            }
            UiAction::Remove(index) => {
                self.report(self.runtime.controller().remove(index));
                let remaining = self.runtime.controller().len();
                self.selected
                    .set(self.selected.get().min(remaining.saturating_sub(1)));
                self.refresh_chain();
                self.refresh_content();
            }
            UiAction::Add(kind) => match self.runtime.controller().add(&kind) {
                Ok(index) => {
                    self.selected.set(index);
                    self.refresh_chain();
                    self.refresh_content();
                    self.toast(&format!("Added {kind}"));
                }
                Err(error) => self.toast(&format!("Could not add {kind}: {error}")),
            },
            UiAction::SetBypassed(bypassed) => {
                self.report(self.runtime.controller().set_bypassed(bypassed));
            }
            UiAction::Clear => {
                self.report(self.runtime.controller().clear());
                self.selected.set(0);
                self.refresh_chain();
                self.refresh_content();
            }
            UiAction::ParametersChanged => {
                // Rebuilding is safe here because the only control that emits
                // this is a button, not a slider the user is mid-drag with.
                self.refresh_chain();
            }
            UiAction::LoadPreset(name) => self.load_preset(&name),
            UiAction::SavePreset => self.prompt_preset_name(),
            UiAction::DeletePreset(name) => self.delete_preset(&name),
            UiAction::SelectDevice(name) => {
                let target = if name.is_empty() { None } else { Some(name) };
                if let Err(error) = self
                    .runtime
                    .restart_with(|config| config.target_sink = target)
                {
                    self.toast(&format!("Could not switch device: {error}"));
                }
            }
            UiAction::RefreshDevices => self.refresh_devices(),
            UiAction::RestartEngine => {
                if let Err(error) = self.runtime.start_engine() {
                    self.toast(&format!("Could not restart the engine: {error}"));
                }
            }
        }
    }

    /// Shows an error rather than losing it.
    fn report(&self, result: Result<(), pipewire_effects_core::CoreError>) {
        if let Err(error) = result {
            self.toast(&error.to_string());
        }
    }

    fn toast(&self, message: &str) {
        self.toast.add_toast(adw::Toast::new(message));
    }

    // ---- Refreshing views ------------------------------------------------

    /// Rebuilds the chain rows.
    fn refresh_chain(&self) {
        clear_rows(&self.chain_group, &self.chain_rows);
        self.runtime.controller().with_model(|model| {
            chain::fill_group(
                &self.chain_group,
                model,
                self.runtime.registry(),
                &self.dispatch(),
                &self.chain_rows,
            );
        });
    }

    /// Rebuilds the preset browser.
    fn refresh_presets(&self) {
        clear_rows(&self.preset_group, &self.preset_rows);

        let infos = self.runtime.presets().list();
        let labels: Vec<&str> = infos.iter().map(|info| info.name.as_str()).collect();
        let model = gtk::StringList::new(&labels);

        let chooser = adw::ComboRow::builder()
            .title("Preset")
            .model(&model)
            .build();
        if infos.is_empty() {
            chooser.set_subtitle("No presets available");
        } else {
            chooser.set_subtitle("Pick one to replace the current chain");
            let dispatch = self.dispatch();
            let names: Vec<String> = infos.iter().map(|info| info.name.clone()).collect();
            chooser.connect_selected_notify(move |row| {
                let index = row.selected() as usize;
                if let Some(name) = names.get(index) {
                    dispatch(UiAction::LoadPreset(name.clone()));
                }
            });
        }
        self.preset_group.add(&chooser);
        self.preset_rows.borrow_mut().push(chooser.clone().upcast());

        let save = adw::ActionRow::builder()
            .title("Save current chain")
            .subtitle("Writes a JSON preset you can edit or share.")
            .activatable(true)
            .build();
        save.add_prefix(&gtk::Image::from_icon_name("document-save-symbolic"));
        {
            let dispatch = self.dispatch();
            save.connect_activated(move |_| dispatch(UiAction::SavePreset));
        }
        self.preset_group.add(&save);
        self.preset_rows.borrow_mut().push(save.upcast());

        // Deleting is only meaningful for presets backed by a file.
        let deletable = infos.iter().find(|info| info.is_deletable()).cloned();
        let remove = adw::ActionRow::builder()
            .title("Delete a preset")
            .subtitle(match &deletable {
                Some(info) => format!("Removes `{}` from your data directory.", info.name),
                None => String::from("You have not saved any presets of your own yet."),
            })
            .activatable(deletable.is_some())
            .build();
        remove.add_prefix(&gtk::Image::from_icon_name("user-trash-symbolic"));
        remove.set_sensitive(deletable.is_some());
        if let Some(info) = deletable {
            let dispatch = self.dispatch();
            remove.connect_activated(move |_| dispatch(UiAction::DeletePreset(info.name.clone())));
        }
        self.preset_group.add(&remove);
        self.preset_rows.borrow_mut().push(remove.upcast());

        *self.presets.borrow_mut() = infos;
    }

    /// Rebuilds the output device chooser.
    fn refresh_devices(&self) {
        clear_rows(&self.device_group, &self.device_rows);

        let sinks = match list_sinks(pipewire_effects_engine::DEFAULT_DISCOVERY_TIMEOUT) {
            Ok(sinks) => sinks,
            Err(error) => {
                tracing::warn!("could not list devices: {error}");
                Vec::new()
            }
        };

        // Index zero means "whatever the system is using", which is the right
        // default: it keeps working when headphones are plugged in.
        let mut labels = vec![String::from("Automatic (system default)")];
        labels.extend(sinks.iter().map(|sink| sink.display_name().to_owned()));

        let current = self.runtime.config().target_sink;
        let selected = current
            .as_deref()
            .and_then(|target| sinks.iter().position(|sink| sink.name == target))
            .map_or(0, |position| position + 1);

        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let model = gtk::StringList::new(&label_refs);
        let chooser = adw::ComboRow::builder()
            .title("Device")
            .model(&model)
            .selected(u32::try_from(selected).unwrap_or(0))
            .build();
        chooser.set_subtitle(if sinks.is_empty() {
            "No output devices found"
        } else {
            "Changing this restarts the audio engine"
        });

        {
            let dispatch = self.dispatch();
            let names: Vec<String> = sinks.iter().map(|sink| sink.name.clone()).collect();
            chooser.connect_selected_notify(move |row| {
                let index = row.selected() as usize;
                let target = if index == 0 {
                    String::new()
                } else {
                    names.get(index - 1).cloned().unwrap_or_default()
                };
                dispatch(UiAction::SelectDevice(target));
            });
        }
        self.device_group.add(&chooser);
        self.device_rows.borrow_mut().push(chooser.upcast());

        let refresh = adw::ActionRow::builder()
            .title("Refresh")
            .subtitle("Re-read the devices PipeWire reports.")
            .activatable(true)
            .build();
        refresh.add_suffix(&gtk::Image::from_icon_name("view-refresh-symbolic"));
        {
            let dispatch = self.dispatch();
            refresh.connect_activated(move |_| dispatch(UiAction::RefreshDevices));
        }
        self.device_group.add(&refresh);
        self.device_rows.borrow_mut().push(refresh.upcast());

        *self.devices.borrow_mut() = sinks;
    }

    /// Rebuilds the detail area for the selected effect.
    fn refresh_content(&self) {
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }

        let index = self.selected.get();
        let page = self.runtime.controller().with_model(|model| {
            model
                .entry(index)
                .map(|entry| editor::build_effect_page(entry, index, &self.dispatch()))
        });

        match page {
            Some(page) => self.content.append(&page),
            None => self.content.append(&editor::build_empty_page()),
        }
    }

    /// Refreshes the engine status rows.
    fn refresh_status(&self) {
        let config = self.runtime.config();
        self.format_row.set_subtitle(&config.format.to_string());
        self.node_row.set_subtitle(&config.capture_node_name);

        let Some(handles) = self.runtime.handles() else {
            self.engine_row.set_subtitle("Stopped");
            self.latency_row.set_subtitle("—");
            return;
        };

        let stats = handles.stats();
        self.latency_row.set_subtitle(&format!(
            "{:.1} ms buffered",
            stats.latency_ms(config.format.sample_rate, config.format.channels)
        ));

        if stats.glitch_samples() == 0 && stats.retired_drops == 0 {
            self.glitch_row.set_subtitle("None");
            self.glitch_row.remove_css_class("error");
        } else {
            self.glitch_row.set_subtitle(&format!(
                "{} samples lost, {} overruns, {} trims",
                stats.glitch_samples(),
                stats.overrun_samples,
                stats.trim_events
            ));
            self.glitch_row.add_css_class("error");
        }
    }

    /// Reads meters and decays the peak hold.
    fn update_meters(&self) {
        let Some(handles) = self.runtime.handles() else {
            return;
        };

        apply_meters(
            &handles.input_meters().snapshot(),
            &self.input_hold,
            &self.input_bars,
            &self.input_readouts,
        );
        apply_meters(
            &handles.output_meters().snapshot(),
            &self.output_hold,
            &self.output_bars,
            &self.output_readouts,
        );
    }

    /// Handles whatever the engine has reported since the last tick.
    fn drain_events(&self) {
        for event in self.runtime.poll_events() {
            match event {
                EngineEvent::Started { capture_node, .. } => {
                    self.engine_row.set_subtitle("Running");
                    self.engine_row.remove_css_class("error");
                    self.node_row.set_subtitle(&capture_node);
                }
                EngineEvent::ConnectionChanged { role, state } => {
                    // Only the playback side gates audible output. The capture
                    // side idles until something plays into the virtual sink,
                    // which is the normal resting state rather than a problem.
                    if role == StreamRole::Playback {
                        self.engine_row.set_subtitle(match state {
                            ConnectionState::Streaming => "Running",
                            ConnectionState::Paused => "Idle",
                            ConnectionState::Connecting => "Connecting…",
                            ConnectionState::Unconnected => "Disconnected",
                            ConnectionState::Error => "Failed",
                        });
                        if state == ConnectionState::Error {
                            self.engine_row.add_css_class("error");
                        } else {
                            self.engine_row.remove_css_class("error");
                        }
                    }
                }
                EngineEvent::FormatNegotiated { role, format } => {
                    if format != self.runtime.config().format {
                        self.toast(&format!(
                            "The {role} stream negotiated {format}, which differs from the \
                             requested format"
                        ));
                    }
                }
                EngineEvent::Failed { message } => {
                    self.engine_row.set_subtitle("Failed");
                    self.engine_row.add_css_class("error");
                    self.toast(&format!("Audio engine failed: {message}"));
                }
                EngineEvent::Stopped => {
                    self.engine_row.set_subtitle("Stopped");
                }
                _ => {}
            }
        }
    }

    // ---- Presets ---------------------------------------------------------

    fn load_preset(&self, name: &str) {
        let preset = match self.runtime.presets().load(name) {
            Ok(preset) => preset,
            Err(error) => {
                self.toast(&format!("Could not load `{name}`: {error}"));
                return;
            }
        };

        let recipe = preset.to_recipe();
        match self.runtime.controller().apply_recipe(&recipe) {
            Ok(report) => {
                if report.is_complete() {
                    self.toast(&format!("Loaded `{}`", preset.name));
                } else {
                    self.toast(&format!(
                        "Loaded `{}` with {} skipped effect(s) and {} unknown setting(s)",
                        preset.name,
                        report.skipped.len(),
                        report.unknown_params.len()
                    ));
                }
            }
            Err(error) => self.toast(&format!("Could not apply `{name}`: {error}")),
        }

        self.selected.set(0);
        self.refresh_chain();
        self.refresh_content();
    }

    fn prompt_preset_name(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };

        let entry = adw::EntryRow::builder().title("Preset name").build();
        entry.set_text(&format!(
            "My chain with {} effect(s)",
            self.runtime.controller().len()
        ));

        let dialog = adw::AlertDialog::new(
            Some("Save preset"),
            Some("Presets live in your data directory as JSON, so you can edit them by hand."),
        );
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("save", "Save");
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        // The closure holds the entry row and a weak reference to the state.
        // It does *not* hold the dialog, so there is no reference cycle even
        // though the entry is also one of the dialog's own children.
        let weak_state = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response != "save" {
                return;
            }
            let Some(state) = weak_state.upgrade() else {
                return;
            };
            state.save_preset(entry.text().trim());
        });

        dialog.present(Some(&window));
    }

    fn save_preset(&self, name: &str) {
        if name.is_empty() {
            self.toast("A preset needs a name");
            return;
        }

        let recipe: ChainRecipe = self.runtime.controller().to_recipe();
        let mut preset = Preset::from_recipe(name, "Saved from the application", &recipe);
        preset.author = String::from("PipewireEffects");

        match self.runtime.presets().save(&preset) {
            Ok(path) => {
                self.toast(&format!("Saved to {}", path.display()));
                self.refresh_presets();
            }
            Err(error) => self.toast(&format!("Could not save the preset: {error}")),
        }
    }

    fn delete_preset(&self, name: &str) {
        match self.runtime.presets().delete(name) {
            Ok(()) => {
                self.toast(&format!("Deleted `{name}`"));
                self.refresh_presets();
            }
            Err(error) => self.toast(&format!("Could not delete `{name}`: {error}")),
        }
    }
}

/// Removes every row a group owns.
fn clear_rows(group: &adw::PreferencesGroup, rows: &Rows) {
    for row in rows.borrow_mut().drain(..) {
        group.remove(&row);
    }
}

/// Publishes meter readings into the bars and labels.
fn apply_meters(
    snapshot: &MeterSnapshot,
    hold: &RefCell<Vec<f32>>,
    bars: &[gtk::LevelBar],
    readouts: &[gtk::Label],
) {
    let mut held = hold.borrow_mut();
    for (channel, bar) in bars.iter().enumerate() {
        let level = snapshot.peak_of(channel);

        // Peak hold decays rather than snapping, so a transient stays readable
        // for a moment instead of flashing past.
        let previous = held.get(channel).copied().unwrap_or(0.0);
        let value = if level > previous {
            level
        } else {
            (previous - METER_DECAY).max(level)
        };
        if let Some(slot) = held.get_mut(channel) {
            *slot = value;
        }

        bar.set_value(meter_fraction(value));
        if let Some(readout) = readouts.get(channel) {
            readout.set_text(&meter_label(level));
        }
    }
}

/// Maps a linear amplitude onto a meter's travel.
///
/// Meters are logarithmic: a linear bar would spend almost all of its length on
/// the top few decibels and show nothing at all for quiet material.
fn meter_fraction(amplitude: f32) -> f64 {
    if !amplitude.is_finite() || amplitude <= 1e-6 {
        return 0.0;
    }
    let db = 20.0 * amplitude.log10();
    f64::from(((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0))
}

/// Formats a level for the meter readout.
fn meter_label(amplitude: f32) -> String {
    if !amplitude.is_finite() || amplitude <= 1e-5 {
        return String::from("—");
    }
    format!("{:.1} dB", 20.0 * amplitude.log10())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_meter_sits_at_zero() {
        assert_eq!(meter_fraction(0.0), 0.0);
        assert_eq!(meter_fraction(1e-9), 0.0);
        assert_eq!(meter_fraction(f32::NAN), 0.0);
        assert_eq!(meter_fraction(f32::INFINITY), 0.0);
    }

    #[test]
    fn full_scale_fills_the_meter() {
        assert!((meter_fraction(1.0) - 1.0).abs() < 1e-9);
        assert!(meter_fraction(4.0) > 1.0 - 1e-9, "clipping pegs the meter");
    }

    #[test]
    fn the_meter_is_scaled_in_decibels_not_amplitude() {
        // Half amplitude is −6 dBFS. On a −60…0 dBFS scale that is 90 % of the
        // travel, not 50 %: a linear meter would spend the whole top half of
        // its length on the top 6 dB and show nothing for quiet material.
        let half = meter_fraction(0.5);
        assert!(
            (0.88..0.92).contains(&half),
            "half amplitude mapped to {half}"
        );

        // The scale is uniform in decibels, so equal dB steps are equal
        // fractions of the travel.
        let at_minus_20 = meter_fraction(10.0_f32.powf(-20.0 / 20.0));
        let at_minus_40 = meter_fraction(10.0_f32.powf(-40.0 / 20.0));
        assert!((at_minus_20 - 2.0 / 3.0).abs() < 1e-5);
        assert!((at_minus_40 - 1.0 / 3.0).abs() < 1e-5);
        assert!((at_minus_20 - at_minus_40 - (at_minus_40 - 0.0)).abs() < 1e-5);
    }

    #[test]
    fn the_meter_floor_is_the_bottom_of_the_travel() {
        let at_floor = meter_fraction(10.0_f32.powf(METER_FLOOR_DB / 20.0));
        assert!(
            at_floor < 1e-6,
            "the floor should sit at zero, got {at_floor}"
        );
    }

    #[test]
    fn quiet_levels_read_as_a_dash_rather_than_minus_infinity() {
        assert_eq!(meter_label(0.0), "—");
        assert_eq!(meter_label(1e-6), "—");
        assert_eq!(meter_label(1.0), "0.0 dB");
        assert!(meter_label(0.5).starts_with("-6.0"));
    }
}
