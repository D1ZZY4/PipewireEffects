use gtk::prelude::*;

use crate::format;

pub fn effect_icon(kind: &str, size: i32) -> gtk::Image {
    let icon = gtk::Image::from_icon_name(format::effect_icon(kind));
    icon.set_pixel_size(size);
    icon.set_valign(gtk::Align::Center);
    icon
}

pub fn menu_entry(label: &str, icon: &str) -> gtk::Button {
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
