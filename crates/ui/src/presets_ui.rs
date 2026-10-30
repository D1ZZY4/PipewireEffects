use std::rc::Rc;

use adw::prelude::*;
use pwe_core::PipelineKind;
use pwe_core::i18n::{t, tr};

use crate::Ui;
use crate::dialogs;
use crate::format;

pub type PresetButtons = (gtk::Button, gtk::Button, gtk::Button, gtk::Button);

pub fn actions_row() -> (gtk::Box, PresetButtons) {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_margin_top(4);
    let save = action_button(format::ICON_SAVE, t("Save the current chain as a preset"));
    save.add_css_class("suggested-action");
    let import = action_button(format::ICON_IMPORT, t("Import a preset"));
    let export = action_button(format::ICON_EXPORT, t("Export the loaded preset"));
    let delete = action_button(format::ICON_REMOVE, t("Delete a user preset"));
    delete.add_css_class("destructive-action");
    bar.append(&save);
    bar.append(&import);
    bar.append(&export);
    bar.append(&delete);
    (bar, (save, delete, import, export))
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
        let row = adw::ActionRow::new();
        row.set_title(&info.name);
        row.set_subtitle(if info.builtin {
            t("Built in")
        } else if info.deletable() {
            t("User preset")
        } else {
            t("System preset")
        });
        row.set_activatable(true);
        row.set_widget_name(&info.name);
        if current.as_deref() == Some(info.name.as_str()) {
            row.add_suffix(&gtk::Image::from_icon_name(format::ICON_COPIED));
        }
        pane.preset_list.append(&row);
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

fn message_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.add_css_class("dim-label");
    row
}

pub fn wire(ui: &Rc<Ui>) {
    for pane in &ui.panes {
        wire_pane(ui, pane);
    }
}

fn wire_pane(ui: &Rc<Ui>, pane: &crate::pages::ChainPane) {
    let kind = pane.kind;
    let (save, delete, import, export) = pane.preset_buttons.clone();

    let ui_load = Rc::clone(ui);
    pane.preset_list.connect_row_activated(move |_, row| {
        let name = row.widget_name().to_string();
        if name.is_empty() {
            return;
        }
        match ui_load.state.borrow_mut().load_preset_section(kind, &name) {
            Ok(warnings) => {
                if !warnings.is_empty() {
                    ui_load.show_banner(&tr(
                        "The preset loaded with warnings: {0}",
                        &[&summarized(&warnings)],
                    ));
                }
                ui_load.rebuild_chain();
                ui_load.rebuild_editor();
                populate(&ui_load);
            }
            Err(error) => ui_load.show_banner(&tr(
                "The preset could not be loaded: {0}",
                &[&error.to_string()],
            )),
        }
    });

    let ui_save = Rc::clone(ui);
    save.connect_clicked(move |_| prompt_save(&ui_save));

    let ui_delete = Rc::clone(ui);
    delete.connect_clicked(move |_| prompt_delete(&ui_delete));

    let ui_import = Rc::clone(ui);
    import.connect_clicked(move |_| {
        let ui = Rc::clone(&ui_import);
        let window = ui.window.clone();
        dialogs::choose_file(
            &window,
            t("Import preset"),
            false,
            None,
            move |path| match ui.state.borrow().store.import(&path, None) {
                Ok((preset, warnings)) => {
                    populate(&ui);
                    let message = imported_banner(&preset.name, &warnings);
                    if warnings.is_empty() {
                        ui.toast(&message);
                    } else {
                        ui.show_banner(&message);
                    }
                }
                Err(error) => ui.show_banner(&tr("Import failed: {0}", &[&error.to_string()])),
            },
        );
    });

    let ui_export = Rc::clone(ui);
    export.connect_clicked(move |_| {
        let name = ui_export.state.borrow().loaded_preset().map(str::to_owned);
        let Some(name) = name else {
            ui_export.show_banner(t("Load a preset before exporting it"));
            return;
        };
        let suggested = format!("{name}.json");
        let ui = Rc::clone(&ui_export);
        let window = ui.window.clone();
        dialogs::choose_file(
            &window,
            t("Export preset"),
            true,
            Some(&suggested),
            move |path| match ui.state.borrow().store.export(&name, &path) {
                Ok(()) => ui.toast(t("Preset exported")),
                Err(error) => {
                    ui.show_banner(&tr("Export failed: {0}", &[&error.to_string()]));
                }
            },
        );
    });
}

pub fn prompt_save(ui: &Rc<Ui>) {
    let kind = ui.active();
    let name = ui
        .state
        .borrow()
        .loaded_preset()
        .map(str::to_owned)
        .unwrap_or_else(|| t("My preset").to_owned());
    let ui = Rc::clone(ui);
    let window = ui.window.clone();
    dialogs::text_prompt(
        &window,
        t("Save preset"),
        t("The current effect chain is stored under this name."),
        &name,
        t("Save"),
        move |name| {
            if name.trim().is_empty() {
                ui.show_banner(t("A preset needs a name"));
                return;
            }
            match ui.state.borrow_mut().save_preset_section(kind, name.trim()) {
                Ok(_) => {
                    populate(&ui);
                    ui.toast(t("Preset saved"));
                }
                Err(error) => {
                    ui.show_banner(&tr(
                        "The preset could not be saved: {0}",
                        &[&error.to_string()],
                    ));
                }
            }
        },
    );
}

fn prompt_delete(ui: &Rc<Ui>) {
    let kind = ui.active();
    let (infos, _) = ui.state.borrow().preset_infos(kind);
    let user_presets: Vec<String> = infos
        .into_iter()
        .filter(|info| info.deletable())
        .map(|info| info.name)
        .collect();
    if user_presets.is_empty() {
        ui.show_banner(t("There are no user presets to delete"));
        return;
    }

    let dialog = adw::Dialog::builder()
        .title(t("Delete preset"))
        .content_width(360)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    for name in &user_presets {
        let row = adw::ActionRow::new();
        row.set_title(name);
        row.set_activatable(true);
        row.set_widget_name(name);
        list.append(&row);
    }
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&list));
    toolbar.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar));

    let ui_confirm = Rc::clone(ui);
    let dialog_window = ui.window.clone();
    let dialog_clone = dialog.clone();
    list.connect_row_activated(move |_, row| {
        let name = row.widget_name().to_string();
        let ui_inner = Rc::clone(&ui_confirm);
        let dialog_clone = dialog_clone.clone();
        dialogs::confirm(
            &dialog_window,
            t("Delete preset?"),
            &tr(
                "'{0}' will be removed from your preset directory.",
                &[&name],
            ),
            t("Delete"),
            move || {
                match ui_inner.state.borrow_mut().delete_preset(&name) {
                    Ok(()) => {
                        populate(&ui_inner);
                        ui_inner.toast(t("Preset deleted"));
                    }
                    Err(error) => {
                        ui_inner.show_banner(&tr("Could not delete: {0}", &[&error.to_string()]))
                    }
                }
                dialog_clone.close();
            },
        );
    });

    dialog.present(Some(&ui.window));
}

fn summarized(warnings: &[String]) -> String {
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

fn imported_banner(name: &str, warnings: &[String]) -> String {
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
