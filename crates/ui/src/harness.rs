use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::Ui;
use crate::dialogs;
use crate::presets_ui;

pub const STATE_VARIABLE: &str = "PIPEWIRE_EFFECTS_UI_STATE";

pub fn install(ui: &Rc<Ui>) {
    let Some(state) = std::env::var(STATE_VARIABLE)
        .ok()
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let ui = Rc::clone(ui);
    glib::timeout_add_seconds_local(2, move || {
        apply(&ui, &state);
        glib::ControlFlow::Break
    });
}

fn apply(ui: &Rc<Ui>, state: &str) {
    match state {
        "chain" => load_default(ui),
        "editor" => {
            load_default(ui);
            select(ui, 2);
        }
        "equalizer" => {
            load_default(ui);
            select_kind(ui, "equalizer");
        }
        "presets" => ui.stack.set_visible_child_name("presets"),
        "devices" => {
            ui.settings_view.expand_diagnostics();
            ui.stack.set_visible_child_name("devices");
        }
        "picker" => open_picker(ui, false),
        "picker-tone" => open_picker(ui, true),
        "about" => reveal_about(ui),
        "save" => presets_ui::prompt_save(ui),
        "banner" => ui.show_banner("No output device matches 'Studio monitors'"),
        "bypass" => ui.bypass.set_active(true),
        "narrow" => {
            load_default(ui);
            ui.window.set_default_size(720, 620);
        }
        _ => {}
    }
}

fn load_default(ui: &Rc<Ui>) {
    match ui.state.borrow_mut().load_preset("Default") {
        Ok(warnings) if warnings.is_empty() => {}
        Ok(warnings) => ui.show_banner(&format!("The preset loaded with warnings: {warnings:?}")),
        Err(error) => {
            ui.show_banner(&format!("The default preset could not be loaded: {error}"));
            return;
        }
    }
    ui.rebuild_chain();
    ui.rebuild_editor();
}

fn select(ui: &Rc<Ui>, index: usize) {
    let id = ui.state.borrow().slots.get(index).map(|slot| slot.id);
    if let Some(id) = id {
        ui.state.borrow_mut().selected = Some(id);
        ui.rebuild_editor();
    }
}

fn select_kind(ui: &Rc<Ui>, kind: &str) {
    let id = ui
        .state
        .borrow()
        .slots
        .iter()
        .find(|slot| slot.kind == kind)
        .map(|slot| slot.id);
    if let Some(id) = id {
        ui.state.borrow_mut().selected = Some(id);
        ui.rebuild_editor();
        ui.rebuild_chain();
    }
}

fn open_picker(ui: &Rc<Ui>, scroll_to_end: bool) {
    let ui = Rc::clone(ui);
    let window = ui.window.clone();
    let dialog = dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
    if !scroll_to_end {
        return;
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
        scroll_to_bottom(dialog.upcast_ref());
    });
}

fn scroll_to_bottom(widget: &gtk::Widget) -> bool {
    if let Some(scrolled) = widget.downcast_ref::<gtk::ScrolledWindow>() {
        let adjustment = scrolled.vadjustment();
        adjustment.set_value(adjustment.upper());
        return true;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if scroll_to_bottom(&current) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn reveal_about(ui: &Rc<Ui>) {
    if let Err(error) = WidgetExt::activate_action(&ui.window, "app.about", None) {
        eprintln!("{STATE_VARIABLE}: the about action could not be activated: {error}");
    }
}
