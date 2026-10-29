use adw::prelude::*;
use gtk::gio;
use pwe_core::i18n::t;

use crate::format;

pub const PAGES: [(&str, &str, &str); 3] = [
    ("page-output", "output", "<primary>1"),
    ("page-input", "input", "<primary>2"),
    ("page-presets", "presets", "<primary>3"),
];

pub const MENU_ACTIONS: [(&str, &str); 2] =
    [("devices", "<primary>4"), ("preferences", "<primary>comma")];

pub const NARROW_LAYOUT: &str = "max-width: 860px";

pub fn dialog(title: &str, page: &adw::PreferencesPage) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder()
        .title(title)
        .content_width(520)
        .content_height(640)
        .follows_content_size(false)
        .build();
    dialog.set_search_enabled(false);
    dialog.add(page);
    dialog
}

pub fn install_dialog(
    application: &adw::Application,
    window: &adw::ApplicationWindow,
    name: &str,
    dialog: &adw::PreferencesDialog,
) {
    let action = gio::SimpleAction::new(name, None);
    let dialog = dialog.clone();
    let window_clone = window.clone();
    action.connect_activate(move |_, _| dialog.present(Some(&window_clone)));
    window.add_action(&action);
    for (menu_name, accelerator) in MENU_ACTIONS {
        if menu_name == name {
            application.set_accels_for_action(&format!("win.{name}"), &[accelerator]);
        }
    }
}

pub fn header_controls(header: &adw::HeaderBar) -> (gtk::ToggleButton, gtk::MenuButton) {
    let bypass = gtk::ToggleButton::new();
    bypass.set_icon_name(format::ICON_BYPASS);
    bypass.set_tooltip_text(Some(t("Bypass the effect chain")));
    bypass.update_property(&[
        gtk::accessible::Property::Label(t("Bypass the effect chain")),
        gtk::accessible::Property::Description(t(
            "Stops the audio from passing through the effects without removing them",
        )),
    ]);
    bypass.add_css_class("flat");
    bypass.add_css_class("bypass-button");
    header.pack_start(&bypass);

    let menu_button = gtk::MenuButton::builder()
        .icon_name(format::ICON_MENU)
        .tooltip_text(t("Main menu"))
        .build();
    header.pack_end(&menu_button);
    (bypass, menu_button)
}

pub fn main_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_item(&menu_item(
        t("Devices"),
        "win.devices",
        format::ICON_PAGE_DEVICES,
    ));
    menu.append_item(&menu_item(
        t("Preferences"),
        "win.preferences",
        format::ICON_PREFERENCES,
    ));
    menu.append(Some(t("About PipeWire Effects")), Some("app.about"));
    menu.append(Some(t("Quit")), Some("app.quit"));
    menu
}

fn menu_item(label: &str, action: &str, icon: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    item.set_icon(&gio::ThemedIcon::new(icon));
    item
}

pub fn placeholder(text: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    label.set_margin_start(18);
    label.set_margin_end(18);
    label.set_margin_top(24);
    label.set_visible(false);
    label.upcast()
}

pub fn install(
    application: &adw::Application,
    window: &adw::ApplicationWindow,
    stack: &adw::ViewStack,
) {
    for (name, page, accelerator) in PAGES {
        let action = gio::SimpleAction::new(name, None);
        let stack = stack.clone();
        action.connect_activate(move |_, _| {
            stack.set_visible_child_name(page);
        });
        window.add_action(&action);
        application.set_accels_for_action(&format!("win.{name}"), &[accelerator]);
    }
}
