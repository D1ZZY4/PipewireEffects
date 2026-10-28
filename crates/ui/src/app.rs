use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use pipewire_effects_core::EffectDef;
use pipewire_effects_engine::EngineConfig;

use crate::runtime::Runtime;
use crate::shell;

pub const APPLICATION_ID: &str = "io.github.D1ZZY4.PipewireEffects";

#[must_use]
pub fn run(registry: &'static [EffectDef], config: EngineConfig) -> std::process::ExitCode {
    let app = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::empty())
        .build();

    let existing: Rc<RefCell<Option<adw::ApplicationWindow>>> = Rc::new(RefCell::new(None));

    app.connect_activate(move |app| {
        if let Some(window) = existing.borrow().as_ref() {
            window.present();
            return;
        }

        let runtime = Runtime::new(registry, config.clone());
        let window = shell::build(Rc::clone(&runtime), app);
        window.present();
        *existing.borrow_mut() = Some(window);
    });

    let code: gtk::glib::ExitCode = app.run();
    std::process::ExitCode::from(u8::from(code))
}
