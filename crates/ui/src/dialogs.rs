use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

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
        .content_width(360)
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
    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_activates_default(true);
    content.append(&label);
    content.append(&entry);
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    let accept_button = gtk::Button::with_label(accept);
    accept_button.add_css_class("suggested-action");
    toolbar.add_bottom_bar(&{
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bar.set_margin_top(6);
        bar.set_margin_bottom(6);
        bar.set_margin_start(6);
        bar.set_margin_end(6);
        bar.set_halign(gtk::Align::Center);
        bar.append(&accept_button);
        let cancel = gtk::Button::with_label("Cancel");
        bar.append(&cancel);
        let dialog_clone = dialog.clone();
        cancel.connect_clicked(move |_| {
            dialog_clone.close();
        });
        bar
    });

    let dialog_clone = dialog.clone();
    let on_accept = Rc::new(on_accept);
    accept_button.connect_clicked(move |_| {
        let value = entry.text().to_string();
        dialog_clone.close();
        on_accept(value);
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

pub fn choose_effect<F>(parent: &impl IsA<gtk::Widget>, on_pick: F)
where
    F: Fn(&'static str) + 'static,
{
    let dialog = adw::Dialog::builder()
        .title("Add effect")
        .content_width(380)
        .content_height(520)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search effects"));
    search.set_margin_start(12);
    search.set_margin_end(12);
    search.set_margin_top(6);
    container.append(&search);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("boxed-list");

    let mut categories: Vec<&'static str> = Vec::new();
    for descriptor in pwe_dsp::EFFECTS {
        let label = descriptor.category.label();
        if !categories.contains(&label) {
            categories.push(label);
        }
    }
    for category in categories {
        let heading = gtk::Label::new(Some(category));
        heading.set_xalign(0.0);
        heading.add_css_class("heading");
        heading.set_margin_top(8);
        heading.set_margin_start(14);
        container.append(&heading);
        for descriptor in pwe_dsp::EFFECTS
            .iter()
            .filter(|d| d.category.label() == category)
        {
            let row = gtk::ListBoxRow::new();
            let row_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            row_box.set_margin_top(8);
            row_box.set_margin_bottom(8);
            row_box.set_margin_start(12);
            row_box.set_margin_end(12);
            let title = gtk::Label::new(Some(descriptor.label));
            title.set_xalign(0.0);
            let subtitle = gtk::Label::new(Some(descriptor.summary));
            subtitle.set_xalign(0.0);
            subtitle.set_wrap(true);
            subtitle.add_css_class("caption");
            subtitle.add_css_class("dim-label");
            row_box.append(&title);
            row_box.append(&subtitle);
            row.set_child(Some(&row_box));
            row.set_widget_name(descriptor.id);
            row.set_activatable(true);
            list.append(&row);
        }
    }

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&list));
    container.append(&scrolled);
    toolbar.set_content(Some(&container));
    dialog.set_child(Some(&toolbar));

    let on_pick = Rc::new(on_pick);
    let dialog_clone = dialog.clone();
    list.connect_row_activated(move |_, row| {
        let name = row.widget_name();
        if let Some(id) = pwe_dsp::EFFECTS
            .iter()
            .find(|descriptor| descriptor.id == name)
        {
            on_pick(id.id);
            dialog_clone.close();
        }
    });

    let list_clone = list.clone();
    search.connect_search_changed(move |entry| {
        let needle = entry.text().to_lowercase();
        let mut child = list_clone.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>() {
                let name = row.widget_name();
                let matches = needle.is_empty()
                    || name.to_lowercase().contains(&needle)
                    || effect_text(&name).to_lowercase().contains(&needle);
                row.set_visible(matches);
            }
            child = next;
        }
    });

    dialog.present(Some(parent));
}

fn effect_text(id: &str) -> String {
    pwe_dsp::find(id).map_or_else(String::new, |descriptor| {
        format!("{} {}", descriptor.label, descriptor.summary)
    })
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
