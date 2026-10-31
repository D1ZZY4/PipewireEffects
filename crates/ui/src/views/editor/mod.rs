use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::t;
use pwe_core::{ParamKind, ParamSet, ParamSpec};

use crate::Ui;
use crate::dialogs;
use crate::format;
use crate::views::response;
use crate::widgets;

mod apo;
mod import;

pub fn build(ui: &Rc<Ui>, slot_kind: &str, params: &ParamSet) -> gtk::Widget {
    let page = adw::PreferencesPage::new();
    page.set_vexpand(true);

    let curve = slot_kind.eq("equalizer").then(|| response::build(params));
    page.add(&hero(ui, slot_kind, params, curve.clone()));

    if let Some(area) = curve.clone() {
        let group = adw::PreferencesGroup::new();
        group.set_title(t("Response"));
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&area));
        frame.add_css_class("response-frame");
        group.add(&frame);
        page.add(&group);
    }

    let mut groups: Vec<&'static str> = Vec::new();
    for spec in params.specs() {
        let group = t(spec.group);
        if !groups.contains(&group) {
            groups.push(group);
        }
    }

    for group_name in groups {
        let group = adw::PreferencesGroup::new();
        group.set_title(if group_name.is_empty() {
            t("Parameters")
        } else {
            group_name
        });
        for spec in params
            .specs()
            .iter()
            .filter(|spec| t(spec.group) == group_name)
        {
            group.add(&row_for(*spec, params, curve.clone()));
        }
        page.add(&group);
    }

    page.upcast()
}

fn hero(
    ui: &Rc<Ui>,
    kind: &str,
    params: &ParamSet,
    curve: Option<gtk::DrawingArea>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::new();
    row.set_title(format::effect_label(kind));
    row.set_subtitle(format::effect_summary(kind));
    row.set_title_lines(1);
    row.add_css_class("hero-row");
    row.add_prefix(&widgets::effect_icon(kind, 20));

    if kind.eq("equalizer") {
        row.add_suffix(&import::button(ui, params, curve.clone()));
    }

    let reset = gtk::Button::from_icon_name(format::ICON_RESET);
    reset.add_css_class("flat");
    reset.set_valign(gtk::Align::Center);
    reset.set_tooltip_text(Some(t("Reset every parameter to its default")));
    let params = params.clone();
    let ui = Rc::clone(ui);
    reset.connect_clicked(move |_| {
        params.reset();
        if let Some(area) = &curve {
            area.queue_draw();
        }
        ui.rebuild_editor();
    });
    row.add_suffix(&reset);
    group.add(&row);
    group
}

pub fn empty_state(ui: &Rc<Ui>) -> gtk::Widget {
    let page = adw::StatusPage::new();
    page.set_icon_name(Some(format::ICON_EMPTY));
    page.set_title(t("No effect selected"));
    page.set_description(Some(t("Add an effect to start shaping your audio.")));
    let button = gtk::Button::with_label(t("Add effect"));
    button.add_css_class("pill");
    button.add_css_class("suggested-action");
    button.set_halign(gtk::Align::Center);
    let ui = Rc::clone(ui);
    button.connect_clicked(move |_| {
        let ui = Rc::clone(&ui);
        let window = ui.window.clone();
        dialogs::choose_effect(&window, move |kind| ui.add_effect(kind));
    });
    page.set_child(Some(&button));
    page.upcast()
}

fn row_for(spec: ParamSpec, params: &ParamSet, curve: Option<gtk::DrawingArea>) -> gtk::Widget {
    match spec.kind {
        ParamKind::Boolean { .. } => boolean_row(spec, params),
        ParamKind::Choice { labels, .. } => choice_row(spec, params, labels),
        ParamKind::Float { .. } => float_row(spec, params, curve),
    }
}

fn boolean_row(spec: ParamSpec, params: &ParamSet) -> gtk::Widget {
    let row = adw::SwitchRow::new();
    row.set_title(t(spec.label));
    row.set_active(
        params
            .get(spec.id)
            .map(pwe_core::ParamValue::as_bool)
            .unwrap_or(false),
    );
    let params = params.clone();
    row.connect_active_notify(move |row| {
        let wanted = row.is_active();
        let current = params
            .get(spec.id)
            .map(pwe_core::ParamValue::as_bool)
            .unwrap_or(false);
        if wanted != current {
            let _ = params.set_bool(spec.id, wanted);
        }
    });
    row.upcast()
}

fn choice_row(spec: ParamSpec, params: &ParamSet, labels: &'static [&'static str]) -> gtk::Widget {
    let options: Vec<&str> = labels.iter().map(|label| t(label)).collect();
    let model = gtk::StringList::new(&options);
    let row = adw::ComboRow::new();
    row.set_title(t(spec.label));
    row.set_model(Some(&model));
    row.set_selected(
        params
            .get(spec.id)
            .map(pwe_core::ParamValue::as_choice)
            .unwrap_or(0) as u32,
    );
    let params = params.clone();
    row.connect_selected_notify(move |row| {
        let wanted = row.selected().min(u32::from(u16::MAX)) as u16;
        let current = params
            .get(spec.id)
            .map(pwe_core::ParamValue::as_choice)
            .unwrap_or(0);
        if wanted != current {
            let _ = params.set_choice(spec.id, wanted);
        }
    });
    row.upcast()
}

fn float_row(spec: ParamSpec, params: &ParamSet, curve: Option<gtk::DrawingArea>) -> gtk::Widget {
    let row = adw::ActionRow::new();
    row.set_title(t(spec.label));
    row.set_title_lines(1);

    let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    suffix.set_valign(gtk::Align::Center);

    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_size_request(120, -1);
    scale.set_round_digits(3);
    let current = params.get(spec.id).unwrap_or_else(|| spec.default_value());
    scale.set_value(f64::from(spec.normalize(current.as_f32())));
    scale.update_property(&[
        gtk::accessible::Property::Label(t(spec.label)),
        gtk::accessible::Property::ValueText(&spec.display(current)),
    ]);

    let value = gtk::Label::new(Some(&spec.display(current)));
    value.set_width_chars(8);
    value.set_xalign(1.0);
    value.add_css_class("numeric");
    value.add_css_class("dim-label");

    suffix.append(&scale);
    suffix.append(&value);
    row.add_suffix(&suffix);

    let params_scale = params.clone();
    let value_label = value.clone();
    let scale_for_notify = scale.clone();
    let curve = curve.clone();
    let notify = Rc::new(move |normalised: f64| {
        let raw = spec.denormalize(normalised as f32);
        if params_scale.set_f32(spec.id, raw).is_ok() {
            if let Some(display) = params_scale.get(spec.id) {
                let text = spec.display(display);
                value_label.set_label(&text);
                scale_for_notify.update_property(&[gtk::accessible::Property::ValueText(&text)]);
            }
            if let Some(area) = &curve {
                area.queue_draw();
            }
        }
    });

    let notify_step = Rc::clone(&notify);
    scale.connect_value_changed(move |scale| notify_step(scale.value()));

    let suffix_for_reset = suffix.clone();
    let params_reset = params.clone();
    let notify_reset = Rc::clone(&notify);
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
    gesture.connect_released(move |_, _, _, _| {
        let default = spec.default_value();
        let _ = params_reset.set(spec.id, default);
        notify_reset(f64::from(spec.normalize(default.as_f32())));
        if let Some(scale) = suffix_for_reset.first_child().and_downcast::<gtk::Scale>() {
            scale.set_value(f64::from(spec.normalize(default.as_f32())));
        }
    });
    suffix.add_controller(gesture);

    row.upcast()
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
