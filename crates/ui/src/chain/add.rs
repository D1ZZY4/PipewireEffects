use std::rc::Rc;

use adw::prelude::*;
use pipewire_effects_core::{EffectCategory, EffectDef};

use crate::action::{Dispatch, UiAction};

pub fn present(parent: &gtk::Window, registry: &'static [EffectDef], dispatch: &Dispatch) {
    let page = adw::PreferencesPage::new();
    for category in EffectCategory::ALL {
        let group = adw::PreferencesGroup::builder()
            .title(category.label())
            .build();
        for definition in registry
            .iter()
            .filter(|definition| definition.category == category)
        {
            group.add(&entry_row(definition, dispatch));
        }
        if group.first_child().is_some() {
            page.add(&group);
        }
    }

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&page)
        .build();

    let dialog = adw::AlertDialog::new(
        Some("Add an effect"),
        Some("It is appended to the end of the chain."),
    );
    dialog.set_extra_child(Some(&scroller));
    dialog.set_content_width(460);
    dialog.set_content_height(520);
    dialog.add_response("close", "Close");
    dialog.set_close_response("close");
    dialog.present(Some(parent));
}

fn entry_row(definition: &EffectDef, dispatch: &Dispatch) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(definition.label)
        .subtitle(definition.summary)
        .activatable(true)
        .build();
    row.add_suffix(&gtk::Image::from_icon_name("list-add-symbolic"));

    let kind = definition.kind.as_str().to_owned();
    let dispatch = Rc::clone(dispatch);
    row.connect_activated(move |row| {
        dispatch(UiAction::Add(kind.clone()));
        if let Some(dialog) = row
            .ancestor(adw::Dialog::static_type())
            .and_downcast::<adw::Dialog>()
        {
            dialog.close();
        }
    });

    row
}
