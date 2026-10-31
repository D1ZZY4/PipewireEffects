use std::rc::Rc;

use gtk::prelude::*;
use pwe_core::i18n::t;
use pwe_engine::EffectSlot;

use crate::Ui;
use crate::format;
use crate::widgets;

pub fn populate(
    ui: &Rc<Ui>,
    list: &gtk::ListBox,
    slots: &[(EffectSlot, &'static str, bool)],
) -> Vec<(EffectSlot, gtk::LevelBar)> {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let mut meters = Vec::with_capacity(slots.len());
    for (id, kind, enabled) in slots {
        let (row, meter) = row(ui, *id, kind, *enabled);
        list.append(&row);
        meters.push((*id, meter));
    }
    meters
}

pub fn row(
    ui: &Rc<Ui>,
    id: EffectSlot,
    kind: &'static str,
    enabled: bool,
) -> (gtk::ListBoxRow, gtk::LevelBar) {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&id.0.to_string());
    row.set_activatable(true);
    row.set_tooltip_text(Some(format::effect_summary(kind)));

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    container.set_margin_top(3);
    container.set_margin_bottom(3);
    container.set_margin_start(4);
    container.set_margin_end(4);

    let icon = widgets::effect_icon(kind, 16);
    if !enabled {
        icon.add_css_class("dim-label");
    }

    let title = gtk::Label::new(Some(format::effect_label(kind)));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("chain-label");
    if !enabled {
        title.add_css_class("dim-label");
    }

    let meter = gtk::LevelBar::new();
    meter.set_min_value(0.0);
    meter.set_max_value(1.0);
    meter.set_value(0.0);
    meter.set_hexpand(true);
    meter.add_css_class("chain-meter");
    meter.set_tooltip_text(Some(t("Level after this effect")));
    meter.update_property(&[gtk::accessible::Property::Label(t(
        "Level after this effect",
    ))]);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&title);
    text.append(&meter);

    let switch = gtk::Switch::new();
    switch.set_active(enabled);
    switch.set_valign(gtk::Align::Center);
    switch.set_tooltip_text(Some(t("Enable or disable this effect")));
    switch.update_property(&[
        gtk::accessible::Property::Label(format::effect_label(kind)),
        gtk::accessible::Property::Description(t("Enable or disable this effect")),
    ]);

    let menu = row_menu(ui, id);

    container.append(&icon);
    container.append(&text);
    container.append(&switch);
    container.append(&menu);

    let ui_switch = Rc::clone(ui);
    switch.connect_active_notify(move |switch| {
        let wanted = switch.is_active();
        let current = ui_switch.state.borrow().slot(id).map(|slot| slot.enabled);
        if current == Some(wanted) {
            return;
        }
        ui_switch.state.borrow_mut().set_enabled(id, wanted);
        ui_switch.rebuild_chain();
    });

    let popup_menu = menu.clone();
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
    gesture.connect_pressed(move |_, _, _, _| popup_menu.popup());
    container.add_controller(gesture);

    row.set_child(Some(&container));
    (row, meter)
}

fn row_menu(ui: &Rc<Ui>, id: EffectSlot) -> gtk::MenuButton {
    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.set_margin_top(6);
    list.set_margin_bottom(6);
    list.set_margin_start(6);
    list.set_margin_end(6);

    let move_up = widgets::menu_entry(t("Move up"), format::ICON_MOVE_UP);
    let move_down = widgets::menu_entry(t("Move down"), format::ICON_MOVE_DOWN);
    let remove = widgets::menu_entry(t("Remove"), format::ICON_REMOVE);
    remove.add_css_class("destructive-action");
    list.append(&move_up);
    list.append(&move_down);
    list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    list.append(&remove);
    popover.set_child(Some(&list));

    let button = gtk::MenuButton::new();
    button.set_icon_name(format::ICON_ROW_MENU);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(t("Reorder or remove this effect")));
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
