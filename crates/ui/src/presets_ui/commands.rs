use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::{t, tr};

use super::{imported_banner, load_section, populate};
use crate::Ui;
use crate::dialogs;

pub fn wire(ui: &Rc<Ui>) {
    for pane in &ui.panes {
        wire_pane(ui, pane);
    }
}

fn wire_pane(ui: &Rc<Ui>, pane: &crate::pages::ChainPane) {
    let kind = pane.kind;
    let (save, delete, import, export, clone) = pane.preset_buttons.clone();

    let ui_load = Rc::clone(ui);
    pane.preset_list.connect_row_activated(move |_, row| {
        let name = row.widget_name().to_string();
        if name.is_empty() {
            return;
        }
        load_section(&ui_load, kind, &name);
    });

    let ui_save = Rc::clone(ui);
    save.connect_clicked(move |_| prompt_save(&ui_save));

    let ui_delete = Rc::clone(ui);
    delete.connect_clicked(move |_| prompt_delete(&ui_delete));

    let ui_clone = Rc::clone(ui);
    clone.connect_clicked(move |_| prompt_clone(&ui_clone));

    let ui_import = Rc::clone(ui);
    import.connect_clicked(move |_| {
        let ui = Rc::clone(&ui_import);
        let window = ui.window.clone();
        dialogs::choose_file(&window, t("Import preset"), false, None, move |path| {
            let imported = ui.state.borrow().store.import(&path, None);
            match imported {
                Ok((preset, warnings)) => {
                    populate(&ui);
                    let message = imported_banner(&preset.name, &warnings);
                    if warnings.is_empty() {
                        ui.toast(&message);
                    } else {
                        ui.show_banner(&message);
                    }
                }
                Err(error) => {
                    ui.show_banner(&tr("Import failed: {0}", &[&error.to_string()]));
                }
            }
        });
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
            move |path| {
                let exported = ui.state.borrow().store.export(&name, &path);
                match exported {
                    Ok(()) => ui.toast(t("Preset exported")),
                    Err(error) => {
                        ui.show_banner(&tr("Export failed: {0}", &[&error.to_string()]));
                    }
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
            let saved = ui.state.borrow_mut().save_preset_section(kind, name.trim());
            match saved {
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

pub(crate) fn prompt_clone(ui: &Rc<Ui>) {
    let source = ui.state.borrow().loaded_preset().map(str::to_owned);
    let Some(source) = source else {
        ui.show_banner(t("Load a preset before cloning it"));
        return;
    };
    let suggested = tr("{0} copy", &[&source]);
    let ui = Rc::clone(ui);
    let window = ui.window.clone();
    dialogs::text_prompt(
        &window,
        t("Clone preset"),
        t("The preset is copied under this name."),
        &suggested,
        t("Clone"),
        move |name| {
            if name.trim().is_empty() {
                ui.show_banner(t("A preset needs a name"));
                return;
            }
            let duplicated = ui.state.borrow().store.duplicate(&source, name.trim());
            match duplicated {
                Ok(_) => {
                    populate(&ui);
                    ui.toast(t("Preset duplicated"));
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

pub fn prompt_delete(ui: &Rc<Ui>) {
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
        .content_width(420)
        .content_height(520)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_margin_top(6);
    list.set_margin_bottom(18);
    list.set_margin_start(12);
    list.set_margin_end(12);
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
                let deleted = ui_inner.state.borrow_mut().delete_preset(&name);
                match deleted {
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
