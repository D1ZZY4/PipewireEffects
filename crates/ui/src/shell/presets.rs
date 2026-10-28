use std::rc::Rc;

use adw::prelude::*;
use pipewire_effects_core::ChainRecipe;
use pipewire_effects_presets::Preset;

use crate::action::UiAction;
use crate::shell::State;

impl State {
    pub fn build_preset_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(8);
        content.set_margin_end(8);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_width_request(260);

        for info in self.runtime.presets().list() {
            list.append(&preset_row(self, &popover, &info));
        }
        content.append(&list);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        let save = gtk::Button::with_label("Save current chain…");
        save.add_css_class("flat");
        save.set_halign(gtk::Align::Fill);
        {
            let state = Rc::clone(self);
            let popover = popover.clone();
            save.connect_clicked(move |_| {
                popover.popdown();
                state.prompt_preset_name();
            });
        }
        content.append(&save);

        popover.set_child(Some(&content));
        popover
    }

    pub fn build_device_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(8);
        content.set_margin_end(8);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_width_request(300);

        let current = self.runtime.config().target_sink;
        let mut names = vec![(String::new(), String::from("Automatic (system default)"))];
        if let Ok(sinks) =
            pipewire_effects_engine::list_sinks(pipewire_effects_engine::DEFAULT_DISCOVERY_TIMEOUT)
        {
            names.extend(sinks.into_iter().map(|sink| {
                let label = sink.display_name().to_owned();
                (sink.name, label)
            }));
        }

        for (node, label) in names {
            let selected =
                current.as_deref() == Some(node.as_str()) || (current.is_none() && node.is_empty());
            let row = gtk::ListBoxRow::new();
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            line.set_margin_top(4);
            line.set_margin_bottom(4);
            line.set_margin_start(4);
            line.set_margin_end(4);

            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_visible(selected);
            check.set_width_request(16);
            let text = gtk::Label::new(Some(&label));
            text.set_xalign(0.0);
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);

            line.append(&check);
            line.append(&text);
            row.set_child(Some(&line));
            list.append(&row);

            let state = Rc::clone(self);
            let popover = popover.clone();
            list.connect_row_activated(move |_, _| {
                popover.popdown();
                state.handle(UiAction::SelectDevice(node.clone()));
            });
        }

        content.append(&list);
        popover.set_child(Some(&content));
        popover
    }

    pub(crate) fn prompt_preset_name(self: &Rc<Self>) {
        let Some(window) = self.window() else {
            return;
        };

        let entry = adw::EntryRow::builder().title("Preset name").build();
        let count = self.runtime.controller().len();
        entry.set_text(&format!("Chain of {count}"));

        let dialog = adw::AlertDialog::new(
            Some("Save preset"),
            Some("Presets are JSON files in your data directory."),
        );
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("save", "Save");
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response != "save" {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            state.save_preset(entry.text().trim());
        });

        dialog.present(Some(&window));
    }

    fn save_preset(self: &Rc<Self>, name: &str) {
        if name.is_empty() {
            self.toast("A preset needs a name");
            return;
        }

        let recipe: ChainRecipe = self.runtime.controller().to_recipe();
        let mut preset = Preset::from_recipe(name, "Saved from the application", &recipe);
        preset.author = String::from("PipewireEffects");

        match self.runtime.presets().save(&preset) {
            Ok(path) => {
                self.toast(&format!("Saved to {}", path.display()));
                self.set_active_preset(Some(String::from(name)));
                self.refresh_presets();
            }
            Err(error) => self.toast(&format!("Could not save the preset: {error}")),
        }
    }
}

fn preset_row(
    state: &Rc<State>,
    popover: &gtk::Popover,
    info: &pipewire_effects_presets::PresetInfo,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    set_activatable(&row, true);

    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.set_margin_top(4);
    line.set_margin_bottom(4);
    line.set_margin_start(4);
    line.set_margin_end(4);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_hexpand(true);

    let title = gtk::Label::new(Some(&info.name));
    title.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some(if info.description.is_empty() {
        if info.builtin { "Built in" } else { "Yours" }
    } else {
        &info.description
    }));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("caption");
    subtitle.add_css_class("dim-label");
    text.append(&title);
    text.append(&subtitle);
    line.append(&text);

    if info.is_deletable() {
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Delete this preset")
            .has_frame(false)
            .build();
        let state = Rc::clone(state);
        let popover = popover.clone();
        let name = info.name.clone();
        remove.connect_clicked(move |_| {
            popover.popdown();
            state.delete_preset(&name);
        });
        line.append(&remove);
    } else {
        let badge = gtk::Label::new(Some("built in"));
        badge.add_css_class("caption");
        badge.add_css_class("dim-label");
        line.append(&badge);
    }

    row.set_child(Some(&line));

    let state = Rc::clone(state);
    let popover = popover.clone();
    let name = info.name.clone();
    row.connect_activate(move |_| {
        popover.popdown();
        state.handle(UiAction::LoadPreset(name.clone()));
    });

    row
}

fn set_activatable(row: &gtk::ListBoxRow, activatable: bool) {
    row.set_activatable(activatable);
    row.set_selectable(false);
}
