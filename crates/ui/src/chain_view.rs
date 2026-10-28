use std::rc::Rc;

use gtk::prelude::*;
use pwe_engine::EffectSlot;

use crate::Ui;
use crate::format;

pub fn row(ui: &Rc<Ui>, id: EffectSlot, kind: &'static str, enabled: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&id.0.to_string());
    row.set_activatable(true);

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    container.set_margin_top(8);
    container.set_margin_bottom(8);
    container.set_margin_start(10);
    container.set_margin_end(8);

    let icon = gtk::Image::from_icon_name(format::category_icon(kind));
    icon.set_valign(gtk::Align::Center);
    icon.add_css_class("dim-label");

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

    container.append(&icon);
    container.append(&text);
    container.append(&toggle);
    container.append(&row_menu(ui, id));
    row.set_child(Some(&container));
    row
}

fn row_menu(ui: &Rc<Ui>, id: EffectSlot) -> gtk::MenuButton {
    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.set_margin_top(6);
    list.set_margin_bottom(6);
    list.set_margin_start(6);
    list.set_margin_end(6);

    let move_up = menu_entry("Move up", "go-up-symbolic");
    let move_down = menu_entry("Move down", "go-down-symbolic");
    let remove = menu_entry("Remove", "user-trash-symbolic");
    remove.add_css_class("destructive-action");
    list.append(&move_up);
    list.append(&move_down);
    list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    list.append(&remove);
    popover.set_child(Some(&list));

    let button = gtk::MenuButton::new();
    button.set_icon_name("view-more-symbolic");
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some("Reorder or remove this effect"));
    button.set_popover(Some(&popover));

    let ui_up = Rc::clone(ui);
    let popover_up = popover.clone();
    move_up.connect_clicked(move |_| {
        ui_up.state.borrow_mut().move_by(id, -1);
        ui_up.rebuild_chain();
        popover_up.popdown();
    });

    let ui_down = Rc::clone(ui);
    let popover_down = popover.clone();
    move_down.connect_clicked(move |_| {
        ui_down.state.borrow_mut().move_by(id, 1);
        ui_down.rebuild_chain();
        popover_down.popdown();
    });

    let ui_remove = Rc::clone(ui);
    let popover_remove = popover.clone();
    remove.connect_clicked(move |_| {
        ui_remove.state.borrow_mut().remove(id);
        ui_remove.rebuild_chain();
        ui_remove.rebuild_editor();
        popover_remove.popdown();
    });

    button
}

fn menu_entry(label: &str, icon: &str) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let image = gtk::Image::from_icon_name(icon);
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    content.append(&image);
    content.append(&text);
    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button
}
