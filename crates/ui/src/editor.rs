use std::rc::Rc;

use adw::prelude::*;
use pwe_core::{ParamKind, ParamSet, ParamSpec};

use crate::response;

pub fn build(slot_kind: &str, params: &ParamSet) -> gtk::Widget {
    let page = adw::PreferencesPage::new();
    page.set_vexpand(true);

    let curve = slot_kind.eq("equalizer").then(|| {
        let area = response::build(params);
        let group = adw::PreferencesGroup::new();
        group.set_title("Response");
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&area));
        frame.add_css_class("response-frame");
        group.add(&frame);
        page.add(&group);
        area
    });

    let mut groups: Vec<&'static str> = Vec::new();
    for spec in params.specs() {
        if !groups.contains(&spec.group) {
            groups.push(spec.group);
        }
    }

    for group_name in groups {
        let group = adw::PreferencesGroup::new();
        group.set_title(if group_name.is_empty() {
            "Parameters"
        } else {
            group_name
        });
        for spec in params
            .specs()
            .iter()
            .filter(|spec| spec.group == group_name)
        {
            group.add(&row_for(*spec, params, curve.clone()));
        }
        page.add(&group);
    }

    let actions = adw::PreferencesGroup::new();
    let reset = gtk::Button::with_label("Reset to defaults");
    reset.add_css_class("destructive-action");
    reset.set_halign(gtk::Align::Start);
    let params = params.clone();
    let curve_clone = curve.clone();
    reset.connect_clicked(move |_| {
        params.reset();
        if let Some(area) = &curve_clone {
            area.queue_draw();
        }
    });
    actions.add(&reset);
    page.add(&actions);

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
    row.set_title(spec.label);
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
    let options: Vec<&str> = labels.to_vec();
    let model = gtk::StringList::new(&options);
    let row = adw::ComboRow::new();
    row.set_title(spec.label);
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
    row.set_title(spec.label);

    let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    suffix.set_valign(gtk::Align::Center);

    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_size_request(160, -1);
    scale.set_round_digits(3);
    let current = params.get(spec.id).unwrap_or_else(|| spec.default_value());
    scale.set_value(f64::from(spec.normalize(current.as_f32())));

    let value = gtk::Label::new(Some(&spec.display(current)));
    value.set_width_chars(9);
    value.set_xalign(1.0);
    value.add_css_class("numeric");
    value.add_css_class("dim-label");

    suffix.append(&scale);
    suffix.append(&value);
    row.add_suffix(&suffix);

    let params_scale = params.clone();
    let value_label = value.clone();
    let curve = curve.clone();
    let notify = Rc::new(move |normalised: f64| {
        let raw = spec.denormalize(normalised as f32);
        if params_scale.set_f32(spec.id, raw).is_ok() {
            if let Some(display) = params_scale.get(spec.id) {
                value_label.set_label(&spec.display(display));
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
mod tests {
    use super::*;

    #[test]
    fn rows_are_built_for_every_parameter_kind() {
        let params = ParamSet::new(vec![
            ParamSpec::float("gain", "Gain", pwe_core::ParamUnit::Decibel)
                .range(-12.0, 12.0, 0.0)
                .build(),
            ParamSpec::boolean("mute", "Mute", false),
            ParamSpec::choice("mode", "Mode", &["a", "b"], 0),
        ]);
        assert_eq!(params.specs().len(), 3);
        assert_eq!(params.get("gain"), Some(pwe_core::ParamValue::Float(0.0)));
    }
}
