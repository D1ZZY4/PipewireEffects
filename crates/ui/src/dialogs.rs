use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::format;

pub fn text_prompt<F>(
    parent: &impl IsA<gtk::Widget>,
    title: &str,
    body: &str,
    initial: &str,
    accept: &str,
    on_accept: F,
) where
    F: Fn(String) + 'static,
{
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(380)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let label = gtk::Label::new(Some(body));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("dim-label");
    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_activates_default(true);
    entry.add_css_class("suggested-action");
    content.append(&label);
    content.append(&entry);
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar.set_halign(gtk::Align::Center);
    let accept_button = gtk::Button::with_label(accept);
    accept_button.add_css_class("suggested-action");
    accept_button.add_css_class("pill");
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("pill");
    bar.append(&cancel);
    bar.append(&accept_button);
    toolbar.add_bottom_bar(&bar);

    let dialog_clone = dialog.clone();
    let on_accept = Rc::new(on_accept);
    accept_button.connect_clicked(move |_| {
        let value = entry.text().to_string();
        dialog_clone.close();
        on_accept(value);
    });

    let dialog_clone = dialog.clone();
    cancel.connect_clicked(move |_| {
        dialog_clone.close();
    });

    dialog.present(Some(parent));
}

pub fn confirm<F>(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    body: &str,
    accept: &str,
    on_accept: F,
) where
    F: Fn() + 'static,
{
    let dialog = adw::AlertDialog::new(Some(title), Some(body));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("accept", accept);
    dialog.set_response_appearance("accept", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let on_accept = Rc::new(on_accept);
    dialog.connect_response(None, move |_, response| {
        if response == "accept" {
            on_accept();
        }
    });
    let widget: &gtk::Widget = parent.as_ref().upcast_ref();
    dialog.present(Some(widget));
}

struct Section {
    container: gtk::Box,
    rows: Vec<adw::ActionRow>,
}

pub fn choose_effect<F>(parent: &impl IsA<gtk::Widget>, on_pick: F)
where
    F: Fn(&'static str) + 'static,
{
    let dialog = adw::Dialog::builder()
        .title("Add effect")
        .content_width(420)
        .content_height(560)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search effects"));
    search.set_margin_top(6);
    search.set_margin_bottom(6);
    search.set_margin_start(12);
    search.set_margin_end(12);
    search.add_css_class("pill");

    let on_pick = Rc::new(on_pick);
    let dialog_clone = dialog.clone();
    let mut sections: Vec<Section> = Vec::new();
    let mut categories: Vec<&'static str> = Vec::new();
    for descriptor in pwe_dsp::EFFECTS {
        if !categories.contains(&descriptor.category.label()) {
            categories.push(descriptor.category.label());
        }
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(6);
    content.set_margin_bottom(18);
    content.set_margin_start(12);
    content.set_margin_end(12);
    for category in categories {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let heading = gtk::Label::new(Some(category));
        heading.set_xalign(0.0);
        heading.add_css_class("heading");
        heading.set_margin_start(6);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        let mut rows = Vec::new();
        for descriptor in pwe_dsp::EFFECTS
            .iter()
            .filter(|descriptor| descriptor.category.label() == category)
        {
            let row = adw::ActionRow::new();
            row.set_title(descriptor.label);
            row.set_subtitle(descriptor.summary);
            row.set_activatable(true);
            row.set_widget_name(descriptor.id);
            let icon = gtk::Image::from_icon_name(format::category_icon(descriptor.id));
            icon.add_css_class("dim-label");
            row.add_prefix(&icon);
            let chevron = gtk::Image::from_icon_name("go-next-symbolic");
            chevron.add_css_class("dim-label");
            row.add_suffix(&chevron);
            list.append(&row);
            rows.push(row);
        }
        let pick = Rc::clone(&on_pick);
        let dialog_clone = dialog_clone.clone();
        list.connect_row_activated(move |_, row| {
            let name = row.widget_name();
            if let Some(descriptor) = pwe_dsp::EFFECTS.iter().find(|d| d.id == name) {
                pick(descriptor.id);
                dialog_clone.close();
            }
        });
        container.append(&heading);
        container.append(&list);
        content.append(&container);
        sections.push(Section { container, rows });
    }

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&content));

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&search);
    layout.append(&scrolled);
    toolbar.set_content(Some(&layout));
    dialog.set_child(Some(&toolbar));

    search.connect_search_changed(move |entry| {
        let needle = entry.text().to_lowercase();
        for section in &sections {
            let mut visible = 0;
            for row in &section.rows {
                let name = row.widget_name();
                let title = row.title().to_lowercase();
                let summary = row.subtitle().unwrap_or_default().to_lowercase();
                let matches = needle.is_empty()
                    || name.to_lowercase().contains(&needle)
                    || title.contains(&needle)
                    || summary.contains(&needle);
                row.set_visible(matches);
                if matches {
                    visible += 1;
                }
            }
            section.container.set_visible(visible > 0);
        }
    });

    dialog.present(Some(parent));
}

pub fn choose_file<F>(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    save: bool,
    suggested: Option<&str>,
    on_file: F,
) where
    F: Fn(std::path::PathBuf) + 'static,
{
    let dialog = gtk::FileDialog::builder().title(title).build();
    if let Some(name) = suggested {
        dialog.set_initial_name(Some(name));
    }
    let on_file = Rc::new(on_file);
    let callback = move |result: Result<gio::File, glib::Error>| {
        if let Ok(file) = result
            && let Some(path) = file.path()
        {
            on_file(path);
        }
    };
    if save {
        dialog.save(Some(parent), gio::Cancellable::NONE, callback);
    } else {
        dialog.open(Some(parent), gio::Cancellable::NONE, callback);
    }
}
