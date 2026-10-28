//! The parameter editor.
//!
//! This module is the payoff of the `ParamSpec` design in
//! `pipewire-effects-core`: it builds a working page of controls for *any*
//! effect, including ones that do not exist yet, without knowing what any of
//! them do. Everything it needs is a label, a type, a range and a unit.
//!
//! Two details are worth pointing at:
//!
//! * **Sliders are driven in normalised space.** A frequency slider is mapped
//!   through [`ParamKind::normalise`], so dragging it feels linear in pitch
//!   rather than in hertz. The `logarithmic` flag on the parameter is what
//!   selects that behaviour, and it applies to any parameter that asks for it.
//! * **Controls write straight to the atomic cell.** Changing a slider needs no
//!   message to the audio thread, no queue and no rebuild — the DSP reads the
//!   new value at the top of the next block.

use std::rc::Rc;

use adw::prelude::*;
use gtk::Orientation;
use pipewire_effects_core::{AtomicParam, EffectModel, ParamKind, ParamSpec, ParamValue};

use crate::action::{Dispatch, UiAction};

/// How many positions a continuous slider offers.
///
/// The scale works in whole steps from zero to this, and the value is mapped
/// through the parameter's own normalisation. A thousand steps is finer than
/// any parameter's display precision, so nothing is lost, and it makes
/// logarithmic mapping and linear mapping use exactly the same code path.
const SLIDER_STEPS: f64 = 1_000.0;

/// Builds the settings page for one effect.
#[must_use]
pub fn build_effect_page(model: &EffectModel, index: usize, dispatch: &Dispatch) -> gtk::Widget {
    let page = adw::PreferencesPage::new();

    page.add(&build_header_group(model, index, dispatch));
    for (title, indices) in group_parameters(model.parameters.specs()) {
        let group = adw::PreferencesGroup::builder().title(&title).build();
        for parameter in indices {
            group.add(&build_row(
                &model.parameters.specs()[parameter],
                &model.parameters.values()[parameter],
            ));
        }
        page.add(&group);
    }

    page.upcast()
}

/// The effect-level controls: enable, reset and remove.
fn build_header_group(
    model: &EffectModel,
    index: usize,
    dispatch: &Dispatch,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(&model.label).build();

    let enable = adw::SwitchRow::builder()
        .title("Enabled")
        .subtitle("Process audio through this effect.")
        .active(model.enabled)
        .build();
    {
        let dispatch = Rc::clone(dispatch);
        enable.connect_active_notify(move |row| {
            dispatch(UiAction::SetEnabled(index, row.is_active()));
        });
    }
    group.add(&enable);

    let sound = adw::ActionRow::builder()
        .title("Current settings")
        .subtitle(model.summary())
        .build();
    group.add(&sound);

    let reset = adw::ActionRow::builder()
        .title("Reset parameters")
        .subtitle("Return every value on this page to its default.")
        .activatable(true)
        .build();
    reset.add_suffix(&gtk::Image::from_icon_name("edit-undo-symbolic"));
    {
        let parameters = model.parameters.clone();
        let dispatch = Rc::clone(dispatch);
        reset.connect_activated(move |_| {
            parameters.reset_to_defaults();
            dispatch(UiAction::ParametersChanged);
        });
    }
    group.add(&reset);

    let remove = adw::ActionRow::builder()
        .title("Remove effect")
        .subtitle("Take this effect out of the chain.")
        .activatable(true)
        .build();
    remove.add_suffix(&gtk::Image::from_icon_name("user-trash-symbolic"));
    remove.add_css_class("error");
    {
        let dispatch = Rc::clone(dispatch);
        remove.connect_activated(move |_| dispatch(UiAction::Remove(index)));
    }
    group.add(&remove);

    group
}

/// Groups parameter identifiers by their prefix.
///
/// Eight equaliser bands produce forty-one parameters, and a flat list of them
/// is unusable. The identifiers already carry the structure — `band1_gain`,
/// `band1_frequency`, and so on — so the grouping needs no extra metadata from
/// the effect: split on the first underscore and use the prefix as the heading.
fn group_parameters(specs: &[ParamSpec]) -> Vec<(String, Vec<usize>)> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();

    for (index, spec) in specs.iter().enumerate() {
        let prefix = spec
            .id
            .as_str()
            .split('_')
            .next()
            .unwrap_or(spec.id.as_str());
        match groups.iter_mut().find(|(name, _)| name == prefix) {
            Some((_, indices)) => indices.push(index),
            None => groups.push((prefix.to_owned(), vec![index])),
        }
    }

    groups
        .into_iter()
        .map(|(prefix, indices)| {
            // If every parameter shares one prefix, a heading would be noise.
            if indices.len() == specs.len() {
                (String::from("Parameters"), indices)
            } else {
                (heading_for(&prefix), indices)
            }
        })
        .collect()
}

/// Turns a parameter prefix into a heading: `band3` becomes `Band 3`.
fn heading_for(prefix: &str) -> String {
    let digits: String = prefix
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let words = &prefix[..prefix.len() - digits.len()];

    let mut heading = String::with_capacity(prefix.len() + 1);
    let mut characters = words.chars();
    if let Some(first) = characters.next() {
        heading.extend(first.to_uppercase());
        heading.push_str(characters.as_str());
    }
    if digits.is_empty() {
        heading
    } else {
        format!("{heading} {digits}")
    }
}

/// Builds the control for one parameter.
fn build_row(spec: &ParamSpec, value: &AtomicParam) -> gtk::Widget {
    match &spec.kind {
        ParamKind::Float { .. } => float_row(spec, value),
        ParamKind::Bool { .. } => bool_row(spec, value),
        ParamKind::Enum { .. } => enum_row(spec, value),
    }
}

fn float_row(spec: &ParamSpec, value: &AtomicParam) -> gtk::Widget {
    let row = adw::ActionRow::builder().title(&spec.label).build();
    if !spec.doc.is_empty() {
        row.set_tooltip_text(Some(&spec.doc));
    }

    let scale = gtk::Scale::with_range(Orientation::Horizontal, 0.0, SLIDER_STEPS, 1.0);
    scale.set_draw_value(false);
    scale.set_size_request(200, -1);
    scale.set_valign(gtk::Align::Center);
    scale.set_value(spec.kind.normalise(value.get()) * SLIDER_STEPS);

    let readout = gtk::Label::new(Some(
        &spec.kind.format_value(ParamValue::Float(value.get())),
    ));
    readout.add_css_class("dim-label");
    readout.set_width_chars(9);
    readout.set_xalign(1.0);
    readout.set_valign(gtk::Align::Center);

    {
        let spec = spec.clone();
        let value = value.clone();
        let readout = readout.clone();
        scale.connect_value_changed(move |scale| {
            let raw = spec.kind.denormalise(scale.value() / SLIDER_STEPS);
            value.set(raw);
            readout.set_text(&spec.kind.format_value(ParamValue::Float(raw)));
        });
    }

    let reset = icon_button("edit-undo-symbolic", "Reset to the default value");
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
        });
    }

    row.add_suffix(&scale);
    row.add_suffix(&readout);
    row.add_suffix(&reset);
    row.upcast()
}

fn bool_row(spec: &ParamSpec, value: &AtomicParam) -> gtk::Widget {
    let row = adw::SwitchRow::builder()
        .title(&spec.label)
        .active(value.get() >= 0.5)
        .build();
    if !spec.doc.is_empty() {
        row.set_subtitle(&spec.doc);
        row.set_tooltip_text(Some(&spec.doc));
    }
    {
        let value = value.clone();
        row.connect_active_notify(move |row| {
            value.set(if row.is_active() { 1.0 } else { 0.0 });
        });
    }
    row.upcast()
}

fn enum_row(spec: &ParamSpec, value: &AtomicParam) -> gtk::Widget {
    let ParamKind::Enum { options, .. } = &spec.kind else {
        // `build_row` only calls this for enumerations, so this is unreachable
        // in practice; returning a harmless row beats panicking in the UI.
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
        row.set_tooltip_text(Some(&spec.doc));
    }
    {
        let value = value.clone();
        row.connect_selected_notify(move |row| {
            #[allow(clippy::cast_precision_loss)]
            value.set(row.selected() as f32);
        });
    }
    row.upcast()
}

/// A flat, frameless button with an icon.
fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("flat");
    button
}

/// A page shown when no effect is selected.
#[must_use]
pub fn build_empty_page() -> gtk::Widget {
    let status = adw::StatusPage::builder()
        .icon_name("audio-x-generic-symbolic")
        .title("No effect selected")
        .description("Pick an effect from the list, or add one to get started.")
        .vexpand(true)
        .build();
    status.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipewire_effects_core::{ParamSpec, ParamUnit};

    #[test]
    fn headings_split_a_trailing_number() {
        assert_eq!(heading_for("band3"), "Band 3");
        assert_eq!(heading_for("preamp"), "Preamp");
        assert_eq!(heading_for("band12"), "Band 12");
        assert_eq!(heading_for(""), "");
    }

    #[test]
    fn parameters_sharing_a_prefix_are_grouped_together() {
        let specs = vec![
            ParamSpec::float(
                "band1_gain",
                "Gain",
                -24.0,
                24.0,
                0.0,
                0.1,
                ParamUnit::Decibel,
            ),
            ParamSpec::float("band1_q", "Q", 0.1, 10.0, 0.7, 0.01, ParamUnit::Quality),
            ParamSpec::float(
                "band2_gain",
                "Gain",
                -24.0,
                24.0,
                0.0,
                0.1,
                ParamUnit::Decibel,
            ),
        ];
        let groups = group_parameters(&specs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "Band 1");
        assert_eq!(groups[0].1, vec![0, 1]);
        assert_eq!(groups[1].0, "Band 2");
        assert_eq!(groups[1].1, vec![2]);
    }

    #[test]
    fn a_single_prefix_across_every_parameter_gets_a_generic_heading() {
        let specs = vec![
            ParamSpec::float("gain", "Gain", -24.0, 24.0, 0.0, 0.1, ParamUnit::Decibel),
            ParamSpec::boolean("gain_enabled", "Enabled", true),
            ParamSpec::float(
                "gain_trim",
                "Trim",
                -12.0,
                12.0,
                0.0,
                0.1,
                ParamUnit::Decibel,
            ),
        ];
        let groups = group_parameters(&specs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "Parameters");
        assert_eq!(groups[0].1, vec![0, 1, 2]);
    }

    #[test]
    fn distinct_prefixes_each_get_their_own_heading() {
        let specs = vec![
            ParamSpec::float("gain", "Gain", -24.0, 24.0, 0.0, 0.1, ParamUnit::Decibel),
            ParamSpec::boolean("mute", "Mute", false),
        ];
        let groups = group_parameters(&specs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "Gain");
        assert_eq!(groups[1].0, "Mute");
    }

    #[test]
    fn every_parameter_lands_in_exactly_one_group() {
        let specs: Vec<ParamSpec> = (0..5)
            .flat_map(|band| {
                [
                    ParamSpec::float(
                        format!("band{band}_freq"),
                        "Frequency",
                        20.0,
                        20_000.0,
                        1_000.0,
                        1.0,
                        ParamUnit::Hertz,
                    ),
                    ParamSpec::boolean(format!("band{band}_on"), "On", true),
                ]
            })
            .collect();

        let mut seen: Vec<usize> = group_parameters(&specs)
            .into_iter()
            .flat_map(|(_, indices)| indices)
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..specs.len()).collect::<Vec<_>>());
    }
}
