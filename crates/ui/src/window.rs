use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use pwe_engine::EffectSlot;

use crate::chain_view;
use crate::dialogs;
use crate::editor;
use crate::format;
use crate::meter::LevelDisplay;
use crate::presets_ui;
use crate::settings::SettingsView;
use crate::state::State;

pub struct Ui {
    pub state: Rc<RefCell<State>>,
    pub window: adw::ApplicationWindow,
    pub chain_list: gtk::ListBox,
    pub editor_host: gtk::Box,
    pub preset_list: gtk::ListBox,
    pub bypass: gtk::Switch,
    pub banner: adw::Banner,
    pub meter_in: LevelDisplay,
    pub meter_out: LevelDisplay,
    pub status_label: gtk::Label,
    pub settings_view: SettingsView,
    pub stack: adw::ViewStack,
}

impl Ui {
    pub fn build(app: &adw::Application, state: State) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("PipeWire Effects")
            .default_width(1040)
            .default_height(720)
            .build();

        let header = adw::HeaderBar::new();
        let bypass = gtk::Switch::new();
        bypass.set_valign(gtk::Align::Center);
        bypass.set_tooltip_text(Some("Bypass the whole effect chain"));
        let bypass_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let bypass_label = gtk::Label::new(Some("Bypass"));
        bypass_label.add_css_class("caption");
        bypass_box.append(&bypass_label);
        bypass_box.append(&bypass);
        header.pack_start(&bypass_box);

        let banner = adw::Banner::new("");
        banner.set_revealed(false);

        let (input_box, meter_in) = LevelDisplay::new("Input");
        let (output_box, meter_out) = LevelDisplay::new("Output");

        let chain_list = gtk::ListBox::new();
        chain_list.set_selection_mode(gtk::SelectionMode::Single);
        chain_list.add_css_class("navigation-sidebar");
        let chain_scroll = gtk::ScrolledWindow::new();
        chain_scroll.set_vexpand(true);
        chain_scroll.set_child(Some(&chain_list));

        let add_button = gtk::Button::with_label("Add effect");
        add_button.add_css_class("pill");
        add_button.add_css_class("suggested-action");
        add_button.set_margin_top(6);

        let status_label = gtk::Label::new(Some("Connecting"));
        status_label.set_xalign(0.0);
        status_label.add_css_class("caption");
        status_label.add_css_class("dim-label");
        status_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sidebar.set_size_request(320, -1);
        sidebar.set_margin_top(10);
        sidebar.set_margin_bottom(10);
        sidebar.set_margin_start(10);
        sidebar.set_margin_end(10);
        sidebar.append(&input_box);
        sidebar.append(&status_label);
        sidebar.append(&chain_scroll);
        sidebar.append(&add_button);

        let editor_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_host.set_vexpand(true);
        let editor_column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        editor_column.set_margin_top(10);
        editor_column.set_margin_bottom(10);
        editor_column.set_margin_start(14);
        editor_column.set_margin_end(14);
        editor_column.append(&output_box);
        editor_column.append(&editor_host);
        let editor_scroll = gtk::ScrolledWindow::new();
        editor_scroll.set_vexpand(true);
        editor_scroll.set_child(Some(&editor_column));

        let effects = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        effects.append(&sidebar);
        effects.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        effects.append(&editor_scroll);

        let (presets_page, preset_list, preset_buttons) = presets_ui::build_page();
        let preset_scroll = gtk::ScrolledWindow::new();
        preset_scroll.set_vexpand(true);
        preset_scroll.set_child(Some(&presets_page));

        let state = Rc::new(RefCell::new(state));
        let target_state = Rc::clone(&state);
        let (settings_page, settings_view) = SettingsView::new(Rc::new(move |target: String| {
            if let Err(error) = target_state.borrow().engine.set_target_device(target) {
                target_state
                    .borrow()
                    .engine
                    .shared()
                    .add_warning(error.to_string());
            }
        }));
        let settings_scroll = gtk::ScrolledWindow::new();
        settings_scroll.set_vexpand(true);
        settings_scroll.set_child(Some(&settings_page));

        let stack = adw::ViewStack::new();
        stack.add_titled_with_icon(
            &effects,
            Some("effects"),
            "Effects",
            "emblem-music-symbolic",
        );
        stack.add_titled_with_icon(
            &preset_scroll,
            Some("presets"),
            "Presets",
            "view-list-symbolic",
        );
        stack.add_titled_with_icon(
            &settings_scroll,
            Some("devices"),
            "Devices",
            "audio-speakers-symbolic",
        );

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        header.set_title_widget(Some(&switcher));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banner);
        toolbar.set_content(Some(&stack));
        window.set_content(Some(&toolbar));

        let ui = Rc::new(Self {
            state,
            window,
            chain_list,
            editor_host,
            preset_list,
            bypass,
            banner,
            meter_in,
            meter_out,
            status_label,
            settings_view,
            stack,
        });

        ui.wire(add_button, preset_buttons);
        presets_ui::populate(&ui);
        ui.rebuild_chain();
        ui.rebuild_editor();
        ui.start_polling();
        ui
    }

    fn wire(
        self: &Rc<Self>,
        add_button: gtk::Button,
        preset_buttons: (gtk::Button, gtk::Button, gtk::Button, gtk::Button),
    ) {
        let ui = Rc::clone(self);
        add_button.connect_clicked(move |_| {
            let ui = Rc::clone(&ui);
            let window = ui.window.clone();
            dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
        });

        let ui = Rc::clone(self);
        self.bypass.connect_active_notify(move |switch| {
            let wanted = switch.is_active();
            if ui.state.borrow().bypassed == wanted {
                return;
            }
            ui.state.borrow_mut().set_bypass(wanted);
        });

        let ui = Rc::clone(self);
        self.chain_list.connect_row_activated(move |_, row| {
            let Ok(value) = row.widget_name().parse::<u64>() else {
                return;
            };
            ui.state.borrow_mut().selected = Some(EffectSlot(value));
            ui.rebuild_editor();
        });

        presets_ui::wire(self, preset_buttons);

        let ui = Rc::clone(self);
        self.banner.connect_button_clicked(move |_| {
            ui.banner.set_revealed(false);
        });
    }

    pub fn add_effect(self: &Rc<Self>, kind: &str) {
        let added = self.state.borrow_mut().add(kind);
        if added.is_none() {
            self.show_banner("The effect could not be added; the chain may be full");
            return;
        }
        self.rebuild_chain();
        self.rebuild_editor();
        self.stack.set_visible_child_name("effects");
    }

    pub fn rebuild_chain(self: &Rc<Self>) {
        while let Some(child) = self.chain_list.first_child() {
            self.chain_list.remove(&child);
        }
        let rows: Vec<(EffectSlot, &'static str, bool)> = self
            .state
            .borrow()
            .slots
            .iter()
            .map(|slot| (slot.id, slot.kind, slot.enabled))
            .collect();
        for (id, kind, enabled) in rows {
            self.chain_list
                .append(&chain_view::row(self, id, kind, enabled));
        }
    }

    pub fn rebuild_editor(self: &Rc<Self>) {
        let (built, selection) = {
            let state = self.state.borrow();
            let built = state.selected.and_then(|id| {
                state
                    .slot(id)
                    .map(|slot| (slot.kind, editor::build(slot.kind, &slot.params)))
            });
            let selection = state
                .slots
                .iter()
                .position(|slot| Some(slot.id) == state.selected);
            (built, selection)
        };
        while let Some(child) = self.editor_host.first_child() {
            self.editor_host.remove(&child);
        }
        match built {
            Some((_kind, widget)) => {
                self.editor_host.append(&widget);
                if let Some(index) = selection
                    && let Some(row) = self.chain_list.row_at_index(index as i32)
                {
                    self.chain_list.select_row(Some(&row));
                }
            }
            None => {
                self.editor_host.append(&empty_state(self));
            }
        }
    }

    pub fn show_banner(&self, message: &str) {
        self.banner.set_title(message);
        self.banner.set_revealed(true);
    }

    pub fn refresh(self: &Rc<Self>) {
        let (snapshot, meters, stats, state) = {
            let state = self.state.borrow();
            let shared = state.engine.shared();
            (
                shared.snapshot(),
                shared.meters().snapshot(),
                shared.stats().snapshot(),
                shared.state(),
            )
        };
        self.meter_in
            .update(meters.input_peak_db, meters.input_rms_db);
        self.meter_out
            .update(meters.output_peak_db, meters.output_rms_db);
        let status = format::status_text(&snapshot, state);
        self.status_label.set_label(&status);
        self.settings_view.update(&snapshot, &stats, state);
        if let Some(error) = self.state.borrow().engine.shared().error() {
            self.banner.set_title(&error);
            self.banner.set_revealed(true);
        }
    }

    fn start_polling(self: &Rc<Self>) {
        let ui = Rc::clone(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            ui.refresh();
            glib::ControlFlow::Continue
        });
    }
}

fn empty_state(ui: &Rc<Ui>) -> gtk::Widget {
    let page = adw::StatusPage::new();
    page.set_icon_name(Some("applications-multimedia-symbolic"));
    page.set_title("No effect selected");
    page.set_description(Some("Add an effect to start shaping your audio."));
    let button = gtk::Button::with_label("Add effect");
    button.add_css_class("pill");
    button.add_css_class("suggested-action");
    button.set_halign(gtk::Align::Center);
    let ui = Rc::clone(ui);
    button.connect_clicked(move |_| {
        let ui = Rc::clone(&ui);
        let window = ui.window.clone();
        dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
    });
    page.set_child(Some(&button));
    page.upcast()
}
