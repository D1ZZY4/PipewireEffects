use std::rc::Rc;

use adw::prelude::*;
use pwe_core::ParamSet;
use pwe_core::i18n::{t, tr};

use crate::Ui;
use crate::dialogs;

use super::apo;

pub fn button(ui: &Rc<Ui>, params: &ParamSet, curve: Option<gtk::DrawingArea>) -> gtk::Button {
    let button = gtk::Button::with_label(t("Import APO"));
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(t(
        "Import an Equalizer APO GraphicEQ, Preamp or Filter file",
    )));
    let ui = Rc::clone(ui);
    let params = params.clone();
    button.connect_clicked(move |_| {
        let window = ui.window.clone();
        let params = params.clone();
        let inner = Rc::clone(&ui);
        let curve = curve.clone();
        dialogs::choose_file(
            &window,
            t("Import an Equalizer APO file"),
            false,
            None,
            move |path| match std::fs::read_to_string(&path) {
                Ok(text) => match apo::apply(&params, &text) {
                    Ok(report) => {
                        if let Some(area) = &curve {
                            area.queue_draw();
                        }
                        inner.rebuild_editor();
                        if report.warnings.is_empty() {
                            let message = tr(
                                "APO import applied {0} entries",
                                &[&report.applied.to_string()],
                            );
                            inner.toast(&message);
                        } else {
                            let message = tr(
                                "APO import applied {0} entries with warnings: {1}",
                                &[&report.applied.to_string(), &report.warnings.join("; ")],
                            );
                            inner.show_banner(&message);
                        }
                    }
                    Err(error) => {
                        let message = tr("APO import failed: {0}", &[&error.to_string()]);
                        inner.show_banner(&message);
                    }
                },
                Err(error) => {
                    let message = tr("APO import failed: {0}", &[&error.to_string()]);
                    inner.show_banner(&message);
                }
            },
        );
    });
    button
}
