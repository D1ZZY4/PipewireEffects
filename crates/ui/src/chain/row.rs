use std::rc::Rc;

use gtk::prelude::*;
use pipewire_effects_core::{EffectCategory, EffectModel};

use crate::action::{Dispatch, UiAction};
use crate::style;

pub fn build_row(
    index: usize,
    entry: &EffectModel,
    total: usize,
    dispatch: &Dispatch,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    style::add(&content, style::EFFECT_ROW);

    let toggle = gtk::Switch::builder()
        .active(entry.enabled)
        .valign(gtk::Align::Center)
        .tooltip_text("Process audio through this effect")
        .build();
    toggle.update_property(&[gtk::accessible::Property::Label(&format!(
        "Enable {}",
        entry.label
    ))]);
    {
        let dispatch = Rc::clone(dispatch);
        toggle.connect_state_set(move |_, state| {
            dispatch(UiAction::SetEnabled(index, state));
            gtk::glib::Propagation::Proceed
        });
    }

    let icon = gtk::Image::from_icon_name(category_icon(entry));
    icon.set_valign(gtk::Align::Center);
    icon.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);

    let name = gtk::Label::new(Some(&entry.label));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let summary = gtk::Label::new(Some(&entry.summary()));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.add_css_class("caption");
    summary.add_css_class("dim-label");
    summary.set_width_chars(8);
    summary.set_max_width_chars(40);

    text.append(&name);
    text.append(&summary);

    content.append(&toggle);
    content.append(&icon);
    content.append(&text);

    let earlier = icon_button("go-up-symbolic", "Move earlier");
    earlier.set_sensitive(index > 0);
    {
        let dispatch = Rc::clone(dispatch);
        earlier.connect_clicked(move |_| dispatch(UiAction::MoveUp(index)));
    }

    let later = icon_button("go-down-symbolic", "Move later");
    later.set_sensitive(index + 1 < total);
    {
        let dispatch = Rc::clone(dispatch);
        later.connect_clicked(move |_| dispatch(UiAction::MoveDown(index)));
    }

    let remove = icon_button("user-trash-symbolic", "Remove");
    {
        let dispatch = Rc::clone(dispatch);
        remove.connect_clicked(move |_| dispatch(UiAction::Remove(index)));
    }

    content.append(&earlier);
    content.append(&later);
    content.append(&remove);

    style::set_inactive(&content, !entry.enabled);
    row.set_child(Some(&content));
    row
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    button
}

fn category_icon(entry: &EffectModel) -> &'static str {
    let category = entry.category;
    match category {
        EffectCategory::Equalizer => "multimedia-equalizer-symbolic",
        EffectCategory::Dynamics => "audio-volume-high-symbolic",
        EffectCategory::Filter => "audio-input-microphone-symbolic",
        EffectCategory::Spatial => "audio-speakers-symbolic",
        EffectCategory::Utility => "preferences-other-symbolic",
    }
}
