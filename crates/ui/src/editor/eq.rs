use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;

use crate::action::Dispatch;
use crate::editor::controls;
use crate::editor::group::{BandIndices, preamp_index};
use crate::widgets::response::BandHandle;
use crate::widgets::{BandStrip, ResponseCurve};

pub struct BandEditor {
    pub curve: gtk::Box,
    pub detail: gtk::Stack,
}

pub fn build_band_editor(
    page: &adw::PreferencesPage,
    model: &pipewire_effects_core::EffectModel,
    bands: &[BandIndices],
    sample_rate: f32,
    dispatch: &Dispatch,
) -> BandEditor {
    let parameters = model.parameters.clone();
    let specs = parameters.specs().to_vec();
    let values = parameters.values().to_vec();

    let curve = ResponseCurve::new();
    let strip = BandStrip::new(bands.len());
    let detail = gtk::Stack::new();
    detail.set_transition_type(gtk::StackTransitionType::Crossfade);
    detail.set_transition_duration(120);

    let selected = Rc::new(Cell::new(0usize));

    let update_strip: Rc<dyn Fn()> = {
        let curve = Rc::clone(&curve);
        let strip = Rc::clone(&strip);
        let parameters = parameters.clone();
        let bands = bands.to_vec();
        Rc::new(move || {
            let mut handles = Vec::with_capacity(bands.len());
            for (position, band) in bands.iter().enumerate() {
                let frequency = parameters.value_f32(band.frequency);
                let gain_db = parameters.value_f32(band.gain);
                let enabled = parameters.value_bool(band.enabled);
                strip.set_band(position, frequency, gain_db, enabled);
                handles.push(BandHandle {
                    frequency,
                    gain_db,
                    enabled,
                });
            }
            curve.set_bands(handles);
            curve.refresh();
        })
    };
    update_strip();

    {
        let curve = Rc::clone(&curve);
        let parameters = parameters.clone();
        let response = model.response;
        curve.set_evaluator(
            Rc::new(move |hz: f32| response.and_then(|f| f(&parameters, hz, sample_rate))),
            sample_rate,
        );
    }

    {
        let detail = detail.clone();
        let curve = Rc::clone(&curve);
        let strip = Rc::clone(&strip);
        strip.set_on_select(Rc::new(move |index| {
            detail.set_visible_child_name(&format!("band{index}"));
            curve.set_selected(index);
        }));
    }

    {
        let curve_for_drag = Rc::clone(&curve);
        let parameters = parameters.clone();
        let bands = bands.to_vec();
        let update_strip = Rc::clone(&update_strip);
        strip.set_on_gain(Rc::new(move |index, gain_db| {
            let Some(band) = bands.get(index) else {
                return;
            };
            parameters.set_f32(band.gain, gain_db);
            curve_for_drag.set_selected(index);
            update_strip();
        }));
    }

    {
        let curve_for_drag = Rc::clone(&curve);
        let parameters = parameters.clone();
        let bands = bands.to_vec();
        let update_strip = Rc::clone(&update_strip);
        strip.set_on_toggle(Rc::new(move |index| {
            let Some(band) = bands.get(index) else {
                return;
            };
            let enabled = !parameters.value_bool(band.enabled);
            parameters
                .set(
                    band.enabled,
                    pipewire_effects_core::ParamValue::Bool(enabled),
                )
                .ok();
            curve_for_drag.set_selected(index);
            update_strip();
        }));
    }

    {
        let parameters = parameters.clone();
        let bands = bands.to_vec();
        let update_strip = Rc::clone(&update_strip);
        let selected = Rc::clone(&selected);
        curve.set_on_drag(Rc::new(move |index, frequency, gain_db| {
            let Some(band) = bands.get(index) else {
                return;
            };
            parameters.set_f32(band.gain, gain_db);
            parameters.set_f32(band.frequency, frequency.clamp(20.0, 20_000.0));
            selected.set(index);
            update_strip();
        }));
    }

    for (position, band) in bands.iter().enumerate() {
        let group = adw::PreferencesGroup::builder()
            .title(format!("Band {}", position + 1))
            .description("The gain is set by dragging the band's bar or point")
            .build();
        group.add(&controls::build_row(
            &specs[band.enabled],
            &values[band.enabled],
            Some(Rc::clone(&update_strip)),
        ));
        group.add(&controls::build_row(
            &specs[band.shape],
            &values[band.shape],
            Some(Rc::clone(&update_strip)),
        ));
        group.add(&controls::build_row(
            &specs[band.frequency],
            &values[band.frequency],
            Some(Rc::clone(&update_strip)),
        ));
        group.add(&controls::build_row(
            &specs[band.quality],
            &values[band.quality],
            Some(Rc::clone(&update_strip)),
        ));
        let band_page = adw::PreferencesPage::new();
        band_page.add(&group);
        detail.add_named(&band_page, Some(&format!("band{position}")));
    }
    detail.set_visible_child_name("band0");
    curve.set_selected(0);
    strip.set_selected(0);

    let curve_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    curve_box.set_margin_top(12);
    curve_box.set_margin_bottom(4);
    curve_box.set_margin_start(12);
    curve_box.set_margin_end(12);
    curve_box.append(curve.widget());
    curve_box.append(strip.widget());

    if let Some(preamp) = preamp_index(&specs) {
        let group = adw::PreferencesGroup::builder().title("Output").build();
        group.add(&controls::build_row(
            &specs[preamp],
            &values[preamp],
            Some(Rc::clone(&update_strip)),
        ));
        page.add(&group);
    }

    let _ = dispatch;

    BandEditor {
        curve: curve_box,
        detail,
    }
}
