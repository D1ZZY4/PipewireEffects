use gtk::prelude::*;

use crate::format;

pub fn effect_icon(kind: &str, size: i32) -> gtk::Image {
    let icon = gtk::Image::from_icon_name(format::effect_icon(kind));
    icon.set_pixel_size(size);
    icon.set_valign(gtk::Align::Center);
    icon
}

pub fn section_title(text: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("heading");
    label.set_margin_start(4);
    label.set_margin_bottom(2);
    label.upcast()
}

pub fn apply_state(icon: &gtk::Image, state: pwe_engine::EngineState) {
    for class in ["success", "warning", "error", "dim-label"] {
        icon.remove_css_class(class);
    }
    let (name, class) = format::state_icon(state);
    icon.set_icon_name(Some(name));
    icon.add_css_class(class);
}
