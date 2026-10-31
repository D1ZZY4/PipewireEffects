#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::Ui;
use crate::dialogs;
use crate::presets;

mod navigation;

use navigation::{
    find_switch_row, open_main_menu, open_preset_menu, scroll_preferences, scroll_to_bottom,
    scroll_to_fraction, set_spectrum,
};

pub const STATE_VARIABLE: &str = "PIPEWIRE_EFFECTS_UI_STATE";
pub const CAPTURE_VARIABLE: &str = "PIPEWIRE_EFFECTS_UI_CAPTURE";
const CAPTURE_ATTEMPTS: u32 = 24;

pub fn install(ui: &Rc<Ui>) {
    let Some(state) = std::env::var(STATE_VARIABLE)
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| apply_size(ui, &value))
    else {
        return;
    };
    match state.as_str() {
        "narrow" => ui.window.set_default_size(720, 620),
        "mobile" | "mobile-drawer" => ui.window.set_default_size(360, 740),
        _ => {}
    }
    let ui = Rc::clone(ui);
    glib::timeout_add_seconds_local(2, move || {
        apply(&ui, &state);
        schedule_capture(&ui);
        glib::ControlFlow::Break
    });
}

fn apply_size(ui: &Rc<Ui>, state: &str) -> String {
    let Some((name, size)) = state.split_once('@') else {
        return state.to_owned();
    };
    let Some((width, height)) = size.split_once('x') else {
        return state.to_owned();
    };
    if let (Ok(width), Ok(height)) = (width.parse::<i32>(), height.parse::<i32>()) {
        ui.window.set_default_size(width, height);
    }
    name.to_owned()
}

fn schedule_capture(ui: &Rc<Ui>) {
    let Some(path) = std::env::var_os(CAPTURE_VARIABLE) else {
        return;
    };
    let path = path.to_string_lossy().into_owned();
    let ui = Rc::clone(ui);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
        attempt_capture(ui, path, 0);
    });
}

fn attempt_capture(ui: Rc<Ui>, path: String, attempt: u32) {
    if capture(&ui, &path) {
        return;
    }
    if attempt >= CAPTURE_ATTEMPTS {
        eprintln!("{CAPTURE_VARIABLE}: gave up after {attempt} attempts");
        return;
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
        attempt_capture(ui, path, attempt + 1);
    });
}

fn capture(ui: &Ui, path: &str) -> bool {
    let window = ui
        .window
        .application()
        .and_then(|application| application.active_window())
        .unwrap_or_else(|| ui.window.clone().upcast::<gtk::Window>());
    let window = window.upcast::<gtk::Widget>();
    let paintable = gtk::WidgetPaintable::new(Some(&window));
    let width = f64::from(window.width());
    let height = f64::from(window.height());
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width, height);
    let Some(node) = snapshot.to_node() else {
        return false;
    };
    let renderer = window.native().and_then(|native| native.renderer());
    let Some(renderer) = renderer else {
        eprintln!("{CAPTURE_VARIABLE}: the window has no renderer");
        return false;
    };
    let texture = renderer.render_texture(&node, None);
    match texture.save_to_png(path) {
        Ok(()) => {
            println!("{CAPTURE_VARIABLE}: wrote {path} at {width}x{height}");
            true
        }
        Err(error) => {
            eprintln!("{CAPTURE_VARIABLE}: {error}");
            false
        }
    }
}

fn apply(ui: &Rc<Ui>, state: &str) {
    if let Some(kind) = state.strip_prefix("effect:") {
        load_default(ui);
        if kind == "echo_canceller" {
            ui.toggle_input(true);
            ui.set_pipeline(pwe_core::PipelineKind::Input);
        }
        ensure_effect(ui, kind);
        select_kind(ui, kind);
        return;
    }
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
        "presets" => load_default(ui),
        "modified" => {
            load_default(ui);
            ensure_effect(ui, "delay");
            ui.rebuild_chain();
        }
        "preset-menu" => {
            load_default(ui);
            ensure_effect(ui, "delay");
            ui.rebuild_chain();
            open_preset_menu(ui);
        }
        "unloaded" => {
            load_default(ui);
            ui.state
                .borrow_mut()
                .unload_preset(pwe_core::PipelineKind::Output);
            ui.rebuild_chain();
            ui.rebuild_editor();
        }
        "clone" => {
            load_default(ui);
            presets::prompt_clone(ui);
        }
        "input" => {
            load_default(ui);
            ui.toggle_input(true);
            ui.set_pipeline(pwe_core::PipelineKind::Input);
            ui.add_effect("gate");
        }
        "devices" => activate(ui, "win.devices"),
        "preferences" => activate(ui, "win.preferences"),
        "preferences-bottom" => {
            activate(ui, "win.preferences");
            scroll_preferences(ui, 1.0);
        }
        "preferences-spectrum" => {
            activate(ui, "win.preferences");
            scroll_preferences(ui, 0.45);
        }
        "developer" => {
            ui.state.borrow_mut().set_developer_mode(true);
            ui.sync_desktop_switches();
            ui.diagnostics_view.expand();
            activate(ui, "win.diagnostics");
            let ui = Rc::clone(ui);
            glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
                if let Some(page) = ui.diagnostics_dialog.visible_page() {
                    scroll_to_bottom(page.upcast_ref());
                }
            });
        }
        "gpu" => {
            load_default(ui);
            ensure_effect(ui, "gain");
            ui.rebuild_chain();
            ui.state.borrow_mut().set_audio_backend("both");
            ui.state.borrow_mut().set_developer_mode(true);
            ui.sync_desktop_switches();
            ui.diagnostics_view.expand();
            activate(ui, "win.diagnostics");
            let ui = Rc::clone(ui);
            glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
                if let Some(page) = ui.diagnostics_dialog.visible_page() {
                    scroll_to_bottom(page.upcast_ref());
                }
            });
        }
        "menu" => {
            load_default(ui);
            open_main_menu(ui);
        }
        "menu-developer" => {
            load_default(ui);
            ui.state.borrow_mut().set_developer_mode(true);
            ui.sync_desktop_switches();
            open_main_menu(ui);
        }
        "dev-live" => {
            load_default(ui);
            activate(ui, "win.preferences");
            let toggler = Rc::clone(ui);
            let closer = Rc::clone(ui);
            let opener = Rc::clone(ui);
            glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
                let dialog = toggler.preferences_dialog.clone();
                if let Some(row) = find_switch_row(dialog.upcast_ref(), "Developer mode") {
                    row.set_active(true);
                }
            });
            glib::timeout_add_local_once(std::time::Duration::from_millis(550), move || {
                closer.preferences_dialog.close();
            });
            glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                open_main_menu(&opener);
            });
        }
        "picker" => open_picker(ui, None),
        "picker-middle" => open_picker(ui, Some(0.27)),
        "picker-tone" => open_picker(ui, Some(1.0)),
        "about" => reveal_about(ui),
        "save" => presets::prompt_save(ui),
        "delete" => {
            let saved = ui
                .state
                .borrow_mut()
                .save_preset_section(pwe_core::PipelineKind::Output, "Delete me");
            let _ = saved;
            presets::prompt_delete(ui);
        }
        "banner" => ui.show_banner("No output device matches 'Studio monitors'"),
        "bypass" => ui.panes[ui.active().index()].effects.set_active(false),
        "spectrum-loud" => {
            load_default(ui);
            ui.state.borrow_mut().set_spectrum_range(2);
        }
        "spectrum-line" => {
            load_default(ui);
            set_spectrum(ui, 1, 2, 2, 1);
        }
        "spectrum-speech" => {
            load_default(ui);
            set_spectrum(ui, 2, 0, 0, 2);
        }
        "narrow" => load_default(ui),
        "mobile" => load_default(ui),
        "mobile-drawer" => {
            load_default(ui);
            ui.split.set_show_sidebar(true);
        }
        "sidebar-hidden" => {
            load_default(ui);
            ui.split.set_show_sidebar(false);
        }
        _ => {}
    }
}

fn load_default(ui: &Rc<Ui>) {
    let loaded = ui.state.borrow_mut().load_preset("Default");
    match loaded {
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

fn ensure_effect(ui: &Rc<Ui>, kind: &str) {
    if !ui
        .state
        .borrow()
        .slots()
        .iter()
        .any(|slot| slot.kind == kind)
    {
        ui.add_effect(kind);
    }
}

fn select(ui: &Rc<Ui>, index: usize) {
    let id = ui.state.borrow().slots().get(index).map(|slot| slot.id);
    if let Some(id) = id {
        ui.state.borrow_mut().set_selected(Some(id));
        ui.rebuild_editor();
    }
}

fn select_kind(ui: &Rc<Ui>, kind: &str) {
    let id = ui
        .state
        .borrow()
        .slots()
        .iter()
        .find(|slot| slot.kind == kind)
        .map(|slot| slot.id);
    if let Some(id) = id {
        ui.state.borrow_mut().set_selected(Some(id));
        ui.rebuild_editor();
        ui.rebuild_chain();
    }
}

fn open_picker(ui: &Rc<Ui>, fraction: Option<f64>) {
    let ui = Rc::clone(ui);
    let window = ui.window.clone();
    let dialog = dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
    let Some(fraction) = fraction else {
        return;
    };
    glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
        scroll_to_fraction(dialog.upcast_ref(), fraction);
    });
}

fn reveal_about(ui: &Rc<Ui>) {
    activate(ui, "app.about");
}

fn activate(ui: &Rc<Ui>, action: &str) {
    if let Err(error) = WidgetExt::activate_action(&ui.window, action, None) {
        eprintln!("{STATE_VARIABLE}: the {action} action could not be activated: {error}");
    }
}
