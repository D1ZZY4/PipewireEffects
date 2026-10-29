use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use pwe_core::PipelineKind;
use pwe_core::i18n::{t, tr};

use crate::state::State;
use crate::window::Ui;

pub const APPLICATION_ID: &str = "io.github.D1ZZY4.PipewireEffects";
pub const ICON_RESOURCE_PATH: &str = "/io/github/D1ZZY4/PipewireEffects/icons/hicolor";
pub const HOMEPAGE: &str = "https://github.com/D1ZZY4/PipewireEffects";

const STYLE: &str = "
.meter-block { padding: 10px 12px; }
.meter { border-radius: 3px; }
.level-bar.meter trough { min-height: 6px; border-radius: 3px; }
.response-frame { border-radius: 12px; }
.response { background: alpha(currentColor, 0.04); border-radius: 12px; }
.numeric { font-feature-settings: \"tnum\"; }

.chain-bypassed { opacity: 0.5; }
.bypass-button:checked { background: alpha(@accent_bg_color, 0.22); color: @accent_color; }
.pipeline-switcher button { font-weight: bold; }
";

#[derive(Debug, Clone)]
pub struct Options {
    pub show_window: bool,
    pub preset: Option<String>,
    pub device: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_window: true,
            preset: None,
            device: None,
        }
    }
}

struct Holder {
    state: Option<State>,
    ui: Option<Rc<Ui>>,
    activations: u32,
    hold: Option<gio::ApplicationHoldGuard>,
}

pub fn run(options: Options) -> std::process::ExitCode {
    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();
    application.connect_startup(|application| {
        install_style();
        install_icons();
        install_actions(application);
    });

    let holder = Rc::new(RefCell::new(Holder {
        state: None,
        ui: None,
        activations: 0,
        hold: None,
    }));

    let active = Rc::clone(&holder);
    let options = Rc::new(options);
    application.connect_activate(move |application| {
        activate(application, &active, &options);
    });

    let result = application.run_with_args::<&str>(&["pipewire-effects"]);
    holder.borrow_mut().ui.take();
    if let Some(state) = holder.borrow_mut().state.as_mut() {
        for kind in pwe_core::ALL_PIPELINES {
            if let Some(engine) = state.chain_mut(kind).engine_mut() {
                engine.shutdown();
            }
        }
    }
    if result == glib::ExitCode::SUCCESS {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn activate(application: &adw::Application, holder: &Rc<RefCell<Holder>>, options: &Rc<Options>) {
    let mut holder = holder.borrow_mut();
    holder.activations += 1;
    if let Some(ui) = &holder.ui {
        ui.window.present();
        return;
    }
    if !options.show_window && holder.activations == 1 {
        match State::new() {
            Ok(mut state) => {
                apply_startup(&mut state, options);
                holder.state = Some(state);
                holder.hold = Some(application.hold());
                println!(
                    "{}",
                    tr("{0} is running without a window", &[APPLICATION_ID])
                );
            }
            Err(error) => {
                show_fatal(application, &error.to_string());
                application.quit();
            }
        }
        return;
    }
    let mut state = match holder.state.take() {
        Some(existing) => existing,
        None => match State::new() {
            Ok(created) => created,
            Err(error) => {
                show_fatal(application, &error.to_string());
                application.quit();
                return;
            }
        },
    };
    apply_startup(&mut state, options);
    let ui = Ui::build(application, state);
    ui.window.present();
    if std::env::var_os("PIPEWIRE_EFFECTS_SMOKE_TEST").is_some() {
        schedule_smoke_test(&ui);
    }
    holder.ui = Some(ui);
}

fn apply_startup(state: &mut State, options: &Options) {
    if let Some(device) = &options.device {
        for kind in pwe_core::ALL_PIPELINES {
            state.chain(kind).set_target_device(device.clone());
        }
    }
    if let Some(preset) = &options.preset {
        match state.load_preset(preset) {
            Ok(warnings) => {
                for warning in warnings {
                    state.warn(&warning);
                }
            }
            Err(error) => state.warn(&tr(
                "'{0}' could not be loaded: {1}",
                &[preset, &error.to_string()],
            )),
        }
    }
}

fn schedule_smoke_test(ui: &Rc<Ui>) {
    let ui = Rc::clone(ui);
    glib::timeout_add_seconds_local(3, move || {
        let mut failures: Vec<String> = Vec::new();
        if !ui.window.is_visible() {
            failures.push("the window is not visible".to_owned());
        }
        ui.add_effect("equalizer");
        let pane = &ui.panes[ui.active().index()];
        if pane.list.first_child().is_none() {
            failures.push("the chain list stayed empty".to_owned());
        }
        if pane.editor_host.first_child().is_none() {
            failures.push("the editor stayed empty".to_owned());
        }
        let loaded = ui.state.borrow_mut().load_preset("Default");
        match loaded {
            Ok(warnings) => {
                ui.rebuild_chain();
                ui.rebuild_editor();
                if !warnings.is_empty() {
                    failures.push(format!("the default preset reported {warnings:?}"));
                }
                let count = ui.state.borrow().slots().len();
                if count == 0 {
                    failures.push("the default preset loaded no effects".to_owned());
                }
            }
            Err(error) => failures.push(format!("the default preset failed: {error}")),
        }
        let (state, devices, rate) = {
            let state = ui.state.borrow();
            let snapshot = state.snapshot_of(PipelineKind::Output);
            (
                state.state_of(PipelineKind::Output),
                snapshot.devices.len(),
                snapshot.sample_rate,
            )
        };
        if devices == 0 {
            failures.push("no PipeWire devices were discovered".to_owned());
        }
        println!(
            "smoke test: state={} devices={} rate={} failures={}",
            state.label(),
            devices,
            rate,
            failures.len()
        );
        for kind in pwe_core::ALL_PIPELINES {
            if let Some(engine) = ui.state.borrow_mut().chain_mut(kind).engine_mut() {
                engine.shutdown();
            }
        }
        if failures.is_empty() {
            std::process::exit(0);
        }
        eprintln!("smoke test failed: {}", failures.join(", "));
        std::process::exit(1);
    });
}

fn install_style() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(STYLE);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn install_icons() {
    if let Err(error) = gio::resources_register_include!("app.gresource") {
        eprintln!("the application resources could not be registered: {error}");
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    theme.add_resource_path(ICON_RESOURCE_PATH);
    let missing = crate::format::missing_icons(&theme);
    if !missing.is_empty() {
        eprintln!("the icon theme is missing: {}", missing.join(", "));
    }
}

fn install_actions(application: &adw::Application) {
    let quit = gio::SimpleAction::new("quit", None);
    let app = application.clone();
    quit.connect_activate(move |_, _| app.quit());
    application.add_action(&quit);
    application.set_accels_for_action("app.quit", &["<primary>q"]);

    let about = gio::SimpleAction::new("about", None);
    let app = application.clone();
    about.connect_activate(move |_, _| {
        let dialog = adw::AboutDialog::builder()
            .application_name("PipeWire Effects")
            .application_icon(APPLICATION_ID)
            .developers(vec![t("PipeWire Effects contributors")])
            .version(env!("CARGO_PKG_VERSION"))
            .license_type(gtk::License::Gpl30)
            .website(HOMEPAGE)
            .issue_url(format!("{HOMEPAGE}/issues"))
            .comments(t(
                "Runs application audio through a chain of effects on a virtual PipeWire sink.",
            ))
            .build();
        dialog.present(app.active_window().as_ref());
    });
    application.add_action(&about);
}

fn show_fatal(application: &adw::Application, message: &str) {
    let dialog = adw::AlertDialog::new(Some(t("PipeWire Effects could not start")), Some(message));
    dialog.add_response("close", t("Close"));
    dialog.set_default_response(Some("close"));
    dialog.set_close_response("close");
    let application = application.clone();
    dialog.connect_response(None, move |_, _| application.quit());
    dialog.present(None::<&gtk::Widget>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_application_icon_resolves_in_the_icon_theme() {
        if gtk::init().is_err() {
            eprintln!("no display is available; the icon lookup test was not executed");
            return;
        }
        gio::resources_register_include!("app.gresource").expect("the resource should register");
        let Some(display) = gtk::gdk::Display::default() else {
            eprintln!("no display is available; the icon lookup test was not executed");
            return;
        };
        let theme = gtk::IconTheme::for_display(&display);
        theme.add_resource_path(ICON_RESOURCE_PATH);
        assert!(
            theme.has_icon(APPLICATION_ID),
            "{APPLICATION_ID} was not found"
        );
        let missing = crate::format::missing_icons(&theme);
        assert!(missing.is_empty(), "the icon theme is missing {missing:?}");
    }

    #[test]
    fn the_default_options_open_a_window() {
        assert!(Options::default().show_window);
        let headless = Options {
            show_window: false,
            ..Options::default()
        };
        assert!(!headless.show_window);
    }
}
