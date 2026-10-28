//! Application bootstrap.
//!
//! Deliberately thin. The application object exists to own the window and the
//! D-Bus name; everything else lives in [`Runtime`] and [`window`].

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use pipewire_effects_core::EffectDef;
use pipewire_effects_engine::EngineConfig;

use crate::runtime::Runtime;
use crate::window;

/// The application id.
///
/// Also the D-Bus name, the desktop file's base name and the icon name, which
/// is why it is a single constant rather than three strings that drift.
pub const APPLICATION_ID: &str = "io.github.D1ZZY4.PipewireEffects";

/// Runs the interface until the last window closes.
///
/// `registry` is the catalogue of effects this build provides, supplied by the
/// caller so that the interface has no compile-time dependency on any
/// particular set of effects.
#[must_use]
pub fn run(registry: &'static [EffectDef], config: EngineConfig) -> std::process::ExitCode {
    let app = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::empty())
        .build();

    let existing: Rc<RefCell<Option<adw::ApplicationWindow>>> = Rc::new(RefCell::new(None));

    app.connect_activate(move |app| {
        // `activate` fires again when a second instance is launched. Presenting
        // the window we already have is the right answer; building a second one
        // would start a second audio engine on the same virtual sink.
        if let Some(window) = existing.borrow().as_ref() {
            window.present();
            return;
        }

        let runtime = Runtime::new(registry, config.clone());
        let window = window::build(Rc::clone(&runtime), app);
        window.present();
        *existing.borrow_mut() = Some(window);
    });

    // The interface converts GLib's exit code so that callers do not have to
    // depend on GLib to launch the application.
    let code: gtk::glib::ExitCode = app.run();
    std::process::ExitCode::from(u8::from(code))
}
