#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::Ui;

pub(super) fn open_preset_menu(ui: &Rc<Ui>) {
    let list = ui.panes[ui.active().index()].preset_list.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
        if let Some(button) = find_menu_button(list.upcast_ref()) {
            button.popup();
        }
    });
}

pub(super) fn open_main_menu(ui: &Rc<Ui>) {
    let window = ui.window.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
        if let Some(button) =
            find_menu_button_by_icon(window.upcast_ref(), crate::format::ICON_MENU)
        {
            button.popup();
        }
    });
}

pub(super) fn find_menu_button_by_icon(
    widget: &gtk::Widget,
    icon: &str,
) -> Option<gtk::MenuButton> {
    if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
        && button.icon_name().as_deref() == Some(icon)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find_menu_button_by_icon(&current, icon) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

pub(super) fn find_menu_button(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
    if let Some(button) = widget.downcast_ref::<gtk::MenuButton>() {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find_menu_button(&current) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

pub(super) fn find_switch_row(widget: &gtk::Widget, title: &str) -> Option<adw::SwitchRow> {
    if let Some(row) = widget.downcast_ref::<adw::SwitchRow>()
        && row.title() == title
    {
        return Some(row.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find_switch_row(&current, title) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

pub(super) fn set_spectrum(ui: &Rc<Ui>, shape: u8, points: u8, height: u8, frequency: u8) {
    let mut state = ui.state.borrow_mut();
    state.set_spectrum_shape(shape);
    state.set_spectrum_points(points);
    state.set_spectrum_height(height);
    state.set_spectrum_frequency_range(frequency);
}

pub(super) fn scroll_to_bottom(widget: &gtk::Widget) -> bool {
    scroll_to_fraction(widget, 1.0)
}

pub(super) fn scroll_to_fraction(widget: &gtk::Widget, fraction: f64) -> bool {
    if let Some(scrolled) = widget.downcast_ref::<gtk::ScrolledWindow>() {
        let adjustment = scrolled.vadjustment();
        let span = (adjustment.upper() - adjustment.page_size()).max(0.0);
        adjustment.set_value(span * fraction.clamp(0.0, 1.0));
        return true;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if scroll_to_fraction(&current, fraction) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}
