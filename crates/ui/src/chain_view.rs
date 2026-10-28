use std::rc::Rc;

use gtk::prelude::*;
use pwe_engine::EffectSlot;

use crate::Ui;
use crate::format;

pub fn row(ui: &Rc<Ui>, id: EffectSlot, kind: &'static str, enabled: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&id.0.to_string());
    row.set_activatable(true);

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    container.set_margin_top(8);
    container.set_margin_bottom(8);
    container.set_margin_start(10);
    container.set_margin_end(6);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_hexpand(true);
    let title = gtk::Label::new(Some(format::effect_label(kind)));
    title.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some(format::effect_summary(kind)));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("caption");
    subtitle.add_css_class("dim-label");
    text.append(&title);
    text.append(&subtitle);

    let toggle = gtk::Switch::new();
    toggle.set_active(enabled);
    toggle.set_valign(gtk::Align::Center);
    toggle.set_tooltip_text(Some("Enable or disable this effect"));
    {
        let ui = Rc::clone(ui);
        toggle.connect_active_notify(move |toggle| {
            let wanted = toggle.is_active();
            let current = ui.state.borrow().slot(id).map(|slot| slot.enabled);
            if current == Some(wanted) {
                return;
            }
            ui.state.borrow_mut().set_enabled(id, wanted);
            ui.rebuild_chain();
        });
    }

    let up = icon_button("go-up-symbolic", "Move up");
    {
        let ui = Rc::clone(ui);
        up.connect_clicked(move |_| {
            ui.state.borrow_mut().move_by(id, -1);
            ui.rebuild_chain();
        });
    }
    let down = icon_button("go-down-symbolic", "Move down");
    {
        let ui = Rc::clone(ui);
        down.connect_clicked(move |_| {
            ui.state.borrow_mut().move_by(id, 1);
            ui.rebuild_chain();
        });
    }
    let remove = icon_button("user-trash-symbolic", "Remove");
    {
        let ui = Rc::clone(ui);
        remove.connect_clicked(move |_| {
            ui.state.borrow_mut().remove(id);
            ui.rebuild_chain();
            ui.rebuild_editor();
        });
    }

    container.append(&text);
    container.append(&toggle);
    container.append(&up);
    container.append(&down);
    container.append(&remove);
    row.set_child(Some(&container));
    row
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button
}
