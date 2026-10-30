use std::rc::Rc;

use adw::prelude::*;
use pwe_core::PipelineKind;
use pwe_core::i18n::{t, tr};

use crate::Ui;
use crate::format;
use crate::widgets;

mod commands;

pub(crate) use commands::{prompt_clone, prompt_delete, prompt_save, wire};

pub type PresetButtons = (
    gtk::Button,
    gtk::Button,
    gtk::Button,
    gtk::Button,
    gtk::Button,
);

pub fn actions_row() -> (gtk::Box, PresetButtons) {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_margin_top(4);
    bar.set_halign(gtk::Align::Center);
    let save = action_button(format::ICON_SAVE, t("Save the current chain as a preset"));
    save.add_css_class("preset-save");
    let import = action_button(format::ICON_IMPORT, t("Import a preset"));
    let export = action_button(format::ICON_EXPORT, t("Export the loaded preset"));
    let clone = action_button(format::ICON_COPY, t("Duplicate the loaded preset"));
    let delete = action_button(format::ICON_REMOVE, t("Delete a user preset"));
    delete.add_css_class("destructive-action");
    bar.append(&save);
    bar.append(&import);
    bar.append(&export);
    bar.append(&clone);
    bar.append(&delete);
    (bar, (save, delete, import, export, clone))
}

fn action_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(tooltip));
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    button
}

pub fn populate(ui: &Rc<Ui>) {
    for pane in &ui.panes {
        populate_pane(ui, pane);
    }
}

fn populate_pane(ui: &Rc<Ui>, pane: &crate::pages::ChainPane) {
    while let Some(child) = pane.preset_list.first_child() {
        pane.preset_list.remove(&child);
    }
    let (infos, warnings) = ui.state.borrow().preset_infos(pane.kind);
    let current = ui.state.borrow().loaded_preset().map(str::to_owned);
    for info in &infos {
        let loaded = current.as_deref() == Some(info.name.as_str());
        let modified = loaded && ui.state.borrow().is_modified(pane.kind);
        pane.preset_list
            .append(&preset_row(ui, pane.kind, info, loaded, modified));
    }
    if pane.kind == PipelineKind::Input && infos.is_empty() {
        pane.preset_list.append(&message_row(
            t("No microphone presets yet"),
            t("Save the input chain to create one"),
        ));
    }
    if !warnings.is_empty() {
        pane.preset_list
            .append(&message_row(t("Skipped files"), &warnings.join("\n")));
    }
}

fn preset_row(
    ui: &Rc<Ui>,
    kind: PipelineKind,
    info: &pwe_presets::PresetInfo,
    loaded: bool,
    modified: bool,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&info.name);
    row.set_activatable(true);
    row.set_tooltip_text(Some(&info.name));

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    container.set_margin_top(3);
    container.set_margin_bottom(3);
    container.set_margin_start(4);
    container.set_margin_end(4);

    let name = gtk::Label::new(Some(&info.name));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_max_width_chars(30);
    name.add_css_class("chain-label");

    let divider = gtk::Label::new(Some("|"));
    divider.add_css_class("dim-label");

    let status = gtk::Label::new(Some(status_text(info)));
    status.add_css_class("dim-label");

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    container.append(&name);
    container.append(&divider);
    container.append(&status);
    container.append(&spacer);
    if modified {
        container.append(&preset_menu(ui, kind, &info.name));
    }
    container.append(&preset_toggle(ui, kind, &info.name, loaded));
    row.set_child(Some(&container));
    row
}

fn status_text(info: &pwe_presets::PresetInfo) -> &'static str {
    if info.builtin {
        t("Built in")
    } else if info.deletable() {
        t("User preset")
    } else {
        t("System preset")
    }
}

fn preset_toggle(ui: &Rc<Ui>, kind: PipelineKind, name: &str, loaded: bool) -> gtk::Button {
    let button = gtk::Button::from_icon_name(format::ICON_COPIED);
    button.add_css_class("circular");
    button.set_valign(gtk::Align::Center);
    let tooltip = if loaded {
        t("Unload this preset")
    } else {
        t("Load this preset")
    };
    button.set_tooltip_text(Some(tooltip));
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    if loaded {
        button.add_css_class("suggested-action");
    } else {
        button.add_css_class("flat");
    }
    let ui = Rc::clone(ui);
    let name = name.to_owned();
    button.connect_clicked(move |_| {
        let current = ui.state.borrow().loaded_preset().map(str::to_owned);
        if current.as_deref() == Some(name.as_str()) {
            unload_section(&ui, kind);
        } else {
            load_section(&ui, kind, &name);
        }
    });
    button
}

fn preset_menu(ui: &Rc<Ui>, kind: PipelineKind, name: &str) -> gtk::MenuButton {
    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.set_margin_top(6);
    list.set_margin_bottom(6);
    list.set_margin_start(6);
    list.set_margin_end(6);

    let update = widgets::menu_entry(t("Update preset"), format::ICON_SAVE);
    let revert = widgets::menu_entry(t("Revert changes"), format::ICON_RESET);
    list.append(&update);
    list.append(&revert);
    popover.set_child(Some(&list));

    let button = gtk::MenuButton::new();
    button.set_icon_name(format::ICON_ROW_MENU);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(t("Update or discard the changes to this preset")));
    button.update_property(&[gtk::accessible::Property::Label(t(
        "Update or discard the changes to this preset",
    ))]);
    button.set_popover(Some(&popover));

    let ui_update = Rc::clone(ui);
    let name_update = name.to_owned();
    let popover_update = popover.clone();
    update.connect_clicked(move |_| {
        let saved = ui_update
            .state
            .borrow_mut()
            .save_preset_section(kind, &name_update);
        match saved {
            Ok(_) => {
                ui_update.rebuild_chain();
                ui_update.rebuild_editor();
                ui_update.toast(t("Preset updated"));
            }
            Err(error) => ui_update.show_banner(&tr(
                "The preset could not be updated: {0}",
                &[&error.to_string()],
            )),
        }
        popover_update.popdown();
    });

    let ui_revert = Rc::clone(ui);
    let name_revert = name.to_owned();
    let popover_revert = popover.clone();
    revert.connect_clicked(move |_| {
        load_section(&ui_revert, kind, &name_revert);
        popover_revert.popdown();
    });

    button
}

pub(crate) fn load_section(ui: &Rc<Ui>, kind: PipelineKind, name: &str) {
    let loaded = ui.state.borrow_mut().load_preset_section(kind, name);
    match loaded {
        Ok(warnings) => {
            if !warnings.is_empty() {
                ui.show_banner(&tr(
                    "The preset loaded with warnings: {0}",
                    &[&summarized(&warnings)],
                ));
            }
            ui.rebuild_chain();
            ui.rebuild_editor();
            populate(ui);
        }
        Err(error) => ui.show_banner(&tr(
            "The preset could not be loaded: {0}",
            &[&error.to_string()],
        )),
    }
}

fn unload_section(ui: &Rc<Ui>, kind: PipelineKind) {
    ui.state.borrow_mut().unload_preset(kind);
    ui.rebuild_chain();
    ui.rebuild_editor();
    populate(ui);
}

fn message_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.add_css_class("dim-label");
    row
}

pub(crate) fn summarized(warnings: &[String]) -> String {
    const SHOWN: usize = 3;
    if warnings.len() <= SHOWN {
        return warnings.join(", ");
    }
    tr(
        "{0} and {1} more",
        &[
            &warnings[..SHOWN].join(", "),
            &(warnings.len() - SHOWN).to_string(),
        ],
    )
}

pub(crate) fn imported_banner(name: &str, warnings: &[String]) -> String {
    if warnings.is_empty() {
        tr("Imported '{0}'", &[name])
    } else {
        tr(
            "Imported '{0}' with warnings: {1}",
            &[name, &summarized(warnings)],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warnings(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("warning {index}")).collect()
    }

    #[test]
    fn a_short_warning_list_is_shown_in_full() {
        assert_eq!(summarized(&warnings(2)), "warning 0, warning 1");
    }

    #[test]
    fn a_long_warning_list_is_summarized_with_a_count() {
        assert_eq!(
            summarized(&warnings(5)),
            "warning 0, warning 1, warning 2 and 2 more"
        );
    }

    #[test]
    fn a_clean_import_is_not_reported_as_a_warning() {
        assert_eq!(imported_banner("Podcast", &[]), "Imported 'Podcast'");
        assert_eq!(
            imported_banner("Podcast", &warnings(1)),
            "Imported 'Podcast' with warnings: warning 0"
        );
    }
}
