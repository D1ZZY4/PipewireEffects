mod controls;
mod eq;
mod group;

use std::rc::Rc;

use adw::prelude::*;
use pipewire_effects_core::EffectModel;

use crate::action::{Dispatch, UiAction};

pub fn build_page(
    model: &EffectModel,
    index: usize,
    sample_rate: f32,
    dispatch: &Dispatch,
) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let page = adw::PreferencesPage::new();
    page.add(&header_group(model, index, dispatch));

    match group::detect_bands(model.parameters.specs()) {
        Some(bands) => {
            let editor = eq::build_band_editor(&page, model, &bands, sample_rate, dispatch);
            root.append(&editor.curve);
            root.append(&page);
            root.append(&editor.detail);
        }
        None => {
            add_generic_groups(&page, model);
            root.append(&page);
        }
    }

    root.upcast()
}

fn header_group(model: &EffectModel, index: usize, dispatch: &Dispatch) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(&model.label).build();

    let enable = adw::SwitchRow::builder()
        .title("Enabled")
        .subtitle("Process audio through this effect")
        .active(model.enabled)
        .build();
    {
        let dispatch = Rc::clone(dispatch);
        enable.connect_active_notify(move |row| {
            dispatch(UiAction::SetEnabled(index, row.is_active()));
        });
    }
    group.add(&enable);

    let reset = adw::ActionRow::builder()
        .title("Reset parameters")
        .subtitle(model.summary())
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

fn add_generic_groups(page: &adw::PreferencesPage, model: &EffectModel) {
    for section in group::group_parameters(model.parameters.specs()) {
        let group = adw::PreferencesGroup::builder()
            .title(&section.title)
            .build();
        for index in section.indices {
            group.add(&controls::build_row(
                &model.parameters.specs()[index],
                &model.parameters.values()[index],
                None,
            ));
        }
        page.add(&group);
    }
}
