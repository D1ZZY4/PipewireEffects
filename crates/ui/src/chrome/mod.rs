#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib::variant::ToVariant;
use pwe_core::i18n::t;

use crate::format;

pub const PAGES: [(&str, &str, &str); 2] = [
    ("page-output", "output", "<primary>1"),
    ("page-input", "input", "<primary>2"),
];

pub const MENU_ACTIONS: [(&str, &str); 3] = [
    ("devices", "<primary>4"),
    ("preferences", "<primary>comma"),
    ("diagnostics", "<primary>5"),
];

pub const NARROW_WIDTH: i32 = 860;
const NARROW_DIALOG_WIDTH: i32 = 340;
const WIDE_DIALOG_WIDTH: i32 = 520;

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
) -> gio::SimpleAction {
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
    action
}

pub fn header_controls(header: &adw::HeaderBar) -> gtk::MenuButton {
    let menu_button = gtk::MenuButton::builder()
        .icon_name(format::ICON_MENU)
        .tooltip_text(t("Main menu"))
        .build();
    header.pack_end(&menu_button);
    menu_button
}

pub fn effects_toggle() -> gtk::ToggleButton {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name(format::ICON_BYPASS));
    content.append(&gtk::Label::new(Some(t("Effects"))));
    let button = gtk::ToggleButton::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button.add_css_class("effects-button");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(t("Bypass the effect chain")));
    button.update_property(&[
        gtk::accessible::Property::Label(t("Effects")),
        gtk::accessible::Property::Description(t(
            "Runs the audio through the effect chain; turn it off to bypass the chain",
        )),
    ]);
    button
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
    let diagnostics = menu_item(
        t("Diagnostics"),
        "win.diagnostics",
        format::ICON_DIAGNOSTICS,
    );
    diagnostics.set_attribute_value("hidden-when", Some(&"action-disabled".to_variant()));
    menu.append_item(&diagnostics);
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

pub fn is_narrow(width: i32) -> bool {
    width > 0 && width <= NARROW_WIDTH
}

fn on_width(apply: impl Fn() + 'static) {
    apply();
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        apply();
        gtk::glib::ControlFlow::Continue
    });
}

pub fn install_adaptive(
    window: &adw::ApplicationWindow,
    split: &adw::OverlaySplitView,
    switcher: &adw::ViewSwitcher,
    dialogs: &[&adw::PreferencesDialog],
) {
    let state: Rc<Cell<Option<bool>>> = Rc::new(Cell::new(None));
    let split = split.clone();
    let switcher = switcher.clone();
    let dialogs: Vec<adw::PreferencesDialog> =
        dialogs.iter().map(|dialog| (*dialog).clone()).collect();
    let window = window.clone();
    on_width(move || {
        let width = if window.width() > 0 {
            window.width()
        } else {
            window.default_width()
        };
        if width <= 0 {
            return;
        }
        let narrow = is_narrow(width);
        if state.get() != Some(narrow) {
            state.set(Some(narrow));
            split.set_collapsed(narrow);
            split.set_show_sidebar(!narrow);
            switcher.set_policy(if narrow {
                adw::ViewSwitcherPolicy::Narrow
            } else {
                adw::ViewSwitcherPolicy::Wide
            });
            let content = if narrow {
                NARROW_DIALOG_WIDTH
            } else {
                WIDE_DIALOG_WIDTH
            };
            for dialog in &dialogs {
                dialog.set_content_width(content);
            }
        }
    });
}

pub fn sidebar_toggle(split: &adw::OverlaySplitView) -> gtk::ToggleButton {
    let toggle = gtk::ToggleButton::new();
    toggle.set_icon_name(format::ICON_SIDEBAR);
    toggle.set_tooltip_text(Some(t("Show or hide the effect chain")));
    toggle.add_css_class("flat");
    toggle.update_property(&[
        gtk::accessible::Property::Label(t("Show or hide the effect chain")),
        gtk::accessible::Property::Description(t(
            "Shows or hides the effect chain and preset sidebar",
        )),
    ]);
    toggle.set_active(split.shows_sidebar());

    let view = split.clone();
    toggle.connect_toggled(move |button| {
        view.set_show_sidebar(button.is_active());
    });

    let button = toggle.clone();
    let split = split.clone();
    split.connect_show_sidebar_notify(move |view| {
        if button.is_active() != view.shows_sidebar() {
            button.set_active(view.shows_sidebar());
        }
    });

    toggle
}
