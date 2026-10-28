//! The chain view: the ordered list of effects, and the browser for adding one.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use pipewire_effects_core::{ChainModel, EffectCategory, EffectDef, EffectModel};

use crate::action::{Dispatch, UiAction};

/// Rows the chain group owns, so they can be removed on refresh.
pub type Rows = Rc<std::cell::RefCell<Vec<gtk::Widget>>>;

/// Fills a group with the rows for the current chain.
///
/// `rows` is the tracking list for the rows this function adds, so the caller
/// can clear them out before the next refresh. Removing and re-adding a handful
/// of rows is far simpler than diffing, and happens only when the user edits
/// the chain.
pub fn fill_group(
    group: &adw::PreferencesGroup,
    model: &ChainModel,
    registry: &'static [EffectDef],
    dispatch: &Dispatch,
    rows: &Rows,
) {
    let total = model.len();
    if total == 0 {
        let empty = adw::ActionRow::builder()
            .title("No effects yet")
            .subtitle("Use “Add effect” below to build a chain.")
            .build();
        empty.add_css_class("dim-label");
        group.add(&empty);
        rows.borrow_mut().push(empty.upcast());
    }

    for (index, entry) in model.entries.iter().enumerate() {
        let row = build_row(index, entry, total, dispatch);
        group.add(&row);
        rows.borrow_mut().push(row.upcast());
    }

    let add = adw::ActionRow::builder()
        .title("Add effect")
        .subtitle("Choose from the effects this build provides.")
        .activatable(true)
        .build();
    add.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
    {
        let dispatch = Rc::clone(dispatch);
        add.connect_activated(move |row| {
            let Some(window) = row
                .root()
                .and_then(|root| root.downcast::<gtk::Window>().ok())
            else {
                return;
            };
            present_add_dialog(&window, registry, &dispatch);
        });
    }
    group.add(&add);
    rows.borrow_mut().push(add.upcast());
}

/// Builds one row of the chain.
fn build_row(
    index: usize,
    entry: &EffectModel,
    total: usize,
    dispatch: &Dispatch,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&entry.label)
        .subtitle(summary_of(entry))
        .activatable(true)
        .build();

    let toggle = gtk::Switch::builder()
        .active(entry.enabled)
        .valign(gtk::Align::Center)
        .tooltip_text("Process audio through this effect")
        .build();
    {
        let dispatch = Rc::clone(dispatch);
        toggle.connect_state_set(move |_, state| {
            dispatch(UiAction::SetEnabled(index, state));
            glib::Propagation::Proceed
        });
    }
    row.add_prefix(&toggle);

    {
        let dispatch = Rc::clone(dispatch);
        row.connect_activated(move |_| dispatch(UiAction::Select(index)));
    }

    let earlier = icon_button("go-up-symbolic", "Move earlier in the chain");
    earlier.set_sensitive(index > 0);
    {
        let dispatch = Rc::clone(dispatch);
        earlier.connect_clicked(move |_| dispatch(UiAction::MoveUp(index)));
    }

    let later = icon_button("go-down-symbolic", "Move later in the chain");
    later.set_sensitive(index + 1 < total);
    {
        let dispatch = Rc::clone(dispatch);
        later.connect_clicked(move |_| dispatch(UiAction::MoveDown(index)));
    }

    let remove = icon_button("user-trash-symbolic", "Remove this effect");
    {
        let dispatch = Rc::clone(dispatch);
        remove.connect_clicked(move |_| dispatch(UiAction::Remove(index)));
    }

    row.add_suffix(&earlier);
    row.add_suffix(&later);
    row.add_suffix(&remove);
    row
}

/// A one-line description of an effect's current settings.
fn summary_of(entry: &EffectModel) -> String {
    let summary = entry.summary();
    if summary.is_empty() {
        String::from("No settings")
    } else {
        summary
    }
}

/// Shows the effect browser.
///
/// The dialog is an `AdwAlertDialog` with a page of grouped rows as its extra
/// child, which avoids writing a custom dialog subclass for what is really just
/// a list.
fn present_add_dialog(parent: &gtk::Window, registry: &'static [EffectDef], dispatch: &Dispatch) {
    let dialog = adw::AlertDialog::new(
        Some("Add effect"),
        Some("The new effect is appended to the end of the chain."),
    );
    dialog.add_response("close", "Cancel");
    dialog.set_close_response("close");
    dialog.set_content_width(460);
    dialog.set_content_height(560);

    let page = adw::PreferencesPage::new();
    for category in EffectCategory::ALL {
        let group = adw::PreferencesGroup::builder()
            .title(category.label())
            .build();
        for definition in registry
            .iter()
            .filter(|definition| definition.category == category)
        {
            group.add(&build_catalogue_row(definition, &dialog, dispatch));
        }
        if group.first_child().is_some() {
            page.add(&group);
        }
    }

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&page)
        .build();
    dialog.set_extra_child(Some(&scroller));
    dialog.present(Some(parent));
}

/// One row in the effect browser.
fn build_catalogue_row(
    definition: &EffectDef,
    dialog: &adw::AlertDialog,
    dispatch: &Dispatch,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(definition.label)
        .subtitle(definition.summary)
        .activatable(true)
        .build();

    let kind = definition.kind.as_str().to_owned();
    let dispatch = Rc::clone(dispatch);
    // A weak reference, so the dialog does not keep itself alive through the
    // closure stored on one of its own children.
    let weak_dialog = dialog.downgrade();
    row.connect_activated(move |_| {
        dispatch(UiAction::Add(kind.clone()));
        if let Some(dialog) = weak_dialog.upgrade() {
            dialog.close();
        }
    });

    row
}

/// A flat, frameless button with an icon.
pub fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("flat");
    button
}
