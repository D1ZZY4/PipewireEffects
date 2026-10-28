use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;

use crate::action::{Dispatch, UiAction};
use crate::shell::State;

pub fn install_actions(window: &adw::ApplicationWindow, state: &Rc<State>, dispatch: &Dispatch) {
    let refresh = gio::SimpleAction::new("refresh-devices", None);
    {
        let dispatch = Rc::clone(dispatch);
        refresh.connect_activate(move |_, _| dispatch(UiAction::RefreshDevices));
    }
    window.add_action(&refresh);

    let restart = gio::SimpleAction::new("restart-engine", None);
    {
        let dispatch = Rc::clone(dispatch);
        restart.connect_activate(move |_, _| dispatch(UiAction::RestartEngine));
    }
    window.add_action(&restart);

    let shortcuts = gio::SimpleAction::new("shortcuts", None);
    {
        let window = window.clone();
        shortcuts.connect_activate(move |_, _| {
            crate::shortcuts::present(&window);
        });
    }
    window.add_action(&shortcuts);

    let about = gio::SimpleAction::new("about", None);
    {
        let weak = Rc::downgrade(state);
        about.connect_activate(move |_, _| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let Some(window) = state.window() else {
                return;
            };
            adw::AboutDialog::builder()
                .application_name("PipewireEffects")
                .application_icon(crate::APPLICATION_ID)
                .version(env!("CARGO_PKG_VERSION"))
                .comments("A fast, modern PipeWire audio effects processor.")
                .website("https://github.com/D1ZZY4/PipewireEffects")
                .license_type(gtk::License::Gpl30)
                .build()
                .present(Some(&window));
        });
    }
    window.add_action(&about);
}
