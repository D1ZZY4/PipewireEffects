#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::rc::Rc;

use adw::prelude::*;

use pwe_engine::EffectSlot;

use crate::dialogs;
use crate::pages::pipeline_of;
use crate::presets;

use super::Ui;

impl Ui {
    pub(super) fn wire(self: &Rc<Self>) {
        for pane in &self.panes {
            let ui = Rc::clone(self);
            pane.add_button.connect_clicked(move |_| {
                let ui = Rc::clone(&ui);
                let window = ui.window.clone();
                dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
            });

            let ui = Rc::clone(self);
            pane.list.connect_row_activated(move |_, row| {
                let Ok(value) = row.widget_name().parse::<u64>() else {
                    return;
                };
                ui.state.borrow_mut().set_selected(Some(EffectSlot(value)));
                ui.rebuild_editor();
            });

            let ui = Rc::clone(self);
            let kind = pane.kind;
            pane.effects.connect_toggled(move |button| {
                let bypassed = !button.is_active();
                if ui.state.borrow().chain(kind).bypassed == bypassed {
                    return;
                }
                if ui.state.borrow().active() != kind {
                    ui.state.borrow_mut().set_active(kind);
                }
                ui.state.borrow_mut().set_bypass(bypassed);
            });
        }

        let ui = Rc::clone(self);
        self.stack.connect_visible_child_name_notify(move |stack| {
            if let Some(name) = stack.visible_child_name()
                && let Some(kind) = pipeline_of(&name)
            {
                ui.set_pipeline(kind);
            }
        });

        presets::wire(self);

        let ui = Rc::clone(self);
        self.banner.connect_button_clicked(move |_| {
            ui.banner.set_revealed(false);
        });
    }
}
