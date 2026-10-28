mod add;
mod row;

use std::rc::Rc;

use gtk::prelude::*;
use pipewire_effects_core::ChainModel;

use crate::action::{Dispatch, UiAction};
pub use add::present as present_add_dialog;

pub fn new_list(dispatch: &Dispatch) -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .build();
    list.add_css_class("boxed-list");
    list.set_valign(gtk::Align::Start);

    let dispatch = Rc::clone(dispatch);
    list.connect_row_selected(move |_, selected| {
        let Some(row) = selected else {
            return;
        };
        #[allow(clippy::cast_sign_loss)]
        let index = row.index().max(0) as usize;
        if row.is_activatable() {
            dispatch(UiAction::Select(index));
        }
    });

    list
}

pub fn fill_list(list: &gtk::ListBox, model: &ChainModel, dispatch: &Dispatch) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    if model.is_empty() {
        list.append(&empty_row());
        return;
    }

    for (index, entry) in model.entries.iter().enumerate() {
        list.append(&row::build_row(index, entry, model.len(), dispatch));
    }
}

fn empty_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.set_halign(gtk::Align::Center);

    let icon = gtk::Image::from_icon_name("audio-x-generic-symbolic");
    icon.set_pixel_size(40);
    icon.add_css_class("dim-label");

    let title = gtk::Label::new(Some("No effects"));
    title.add_css_class("heading");

    let hint = gtk::Label::new(Some("Add one to start shaping the sound"));
    hint.add_css_class("dim-label");
    hint.set_wrap(true);
    hint.set_max_width_chars(24);
    hint.set_justify(gtk::Justification::Center);

    content.append(&icon);
    content.append(&title);
    content.append(&hint);
    row.set_child(Some(&content));
    row
}
