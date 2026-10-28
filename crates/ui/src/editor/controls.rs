use std::rc::Rc;

use adw::prelude::*;
use gtk::Orientation;
use pipewire_effects_core::{AtomicParam, ParamKind, ParamSpec, ParamValue};

use crate::style;

const SLIDER_STEPS: f64 = 1_000.0;

pub type OnChange = Rc<dyn Fn()>;

pub fn build_row(
    spec: &ParamSpec,
    value: &AtomicParam,
    on_change: Option<OnChange>,
) -> gtk::Widget {
    match &spec.kind {
        ParamKind::Float { .. } => float_row(spec, value, on_change),
        ParamKind::Bool { .. } => bool_row(spec, value, on_change),
        ParamKind::Enum { .. } => enum_row(spec, value, on_change),
    }
}

fn float_row(spec: &ParamSpec, value: &AtomicParam, on_change: Option<OnChange>) -> gtk::Widget {
    let row = adw::ActionRow::builder().title(&spec.label).build();
    if !spec.doc.is_empty() {
        row.set_tooltip_text(Some(&spec.doc));
    }

    let scale = gtk::Scale::with_range(Orientation::Horizontal, 0.0, SLIDER_STEPS, 1.0);
    scale.set_draw_value(false);
    scale.set_size_request(220, -1);
    scale.set_valign(gtk::Align::Center);
    scale.set_value(spec.kind.normalise(value.get()) * SLIDER_STEPS);
    scale.update_property(&[gtk::accessible::Property::Label(&spec.label)]);

    let readout = gtk::Label::new(Some(
        &spec.kind.format_value(ParamValue::Float(value.get())),
    ));
    style::add(&readout, style::NUMERIC);
    readout.set_width_chars(9);
    readout.set_xalign(1.0);
    readout.set_valign(gtk::Align::Center);

    {
        let spec = spec.clone();
        let value = value.clone();
        let readout = readout.clone();
        let on_change = on_change.clone();
        scale.connect_value_changed(move |scale| {
            let raw = spec.kind.denormalise(scale.value() / SLIDER_STEPS);
            value.set(raw);
            readout.set_text(&spec.kind.format_value(ParamValue::Float(raw)));
            if let Some(on_change) = &on_change {
                on_change();
            }
        });
    }

    let reset = icon_button("edit-undo-symbolic", "Reset to the default");
    {
        let spec = spec.clone();
        let value = value.clone();
        let scale = scale.clone();
        let readout = readout.clone();
        reset.connect_clicked(move |_| {
            let default = spec.default_value().to_f32();
            value.set(default);
            scale.set_value(spec.kind.normalise(default) * SLIDER_STEPS);
            readout.set_text(&spec.kind.format_value(ParamValue::Float(default)));
            if let Some(on_change) = &on_change {
                on_change();
            }
        });
    }

    row.add_suffix(&scale);
    row.add_suffix(&readout);
    row.add_suffix(&reset);
    row.upcast()
}

fn bool_row(spec: &ParamSpec, value: &AtomicParam, on_change: Option<OnChange>) -> gtk::Widget {
    let row = adw::SwitchRow::builder()
        .title(&spec.label)
        .active(value.get() >= 0.5)
        .build();
    if !spec.doc.is_empty() {
        row.set_subtitle(&spec.doc);
    }
    {
        let value = value.clone();
        row.connect_active_notify(move |row| {
            value.set(if row.is_active() { 1.0 } else { 0.0 });
            if let Some(on_change) = &on_change {
                on_change();
            }
        });
    }
    row.upcast()
}

fn enum_row(spec: &ParamSpec, value: &AtomicParam, on_change: Option<OnChange>) -> gtk::Widget {
    let ParamKind::Enum { options, .. } = &spec.kind else {
        return adw::ActionRow::builder()
            .title(&spec.label)
            .build()
            .upcast();
    };

    let labels: Vec<&str> = options.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&labels);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let current = (value.get().max(0.0) as u32).min(model.n_items().saturating_sub(1));

    let row = adw::ComboRow::builder()
        .title(&spec.label)
        .model(&model)
        .selected(current)
        .build();
    if !spec.doc.is_empty() {
        row.set_subtitle(&spec.doc);
    }
    {
        let value = value.clone();
        row.connect_selected_notify(move |row| {
            #[allow(clippy::cast_precision_loss)]
            value.set(row.selected() as f32);
            if let Some(on_change) = &on_change {
                on_change();
            }
        });
    }
    row.upcast()
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    button
}
