mod actions;
mod handle;
mod presets;
mod refresh;
mod state;

use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;

pub use state::State;

use crate::action::{Dispatch, UiAction};
use crate::runtime::Runtime;
use crate::style;
use crate::widgets::Meter;

const SIDEBAR_WIDTH: i32 = 330;
const METER_WIDTH: i32 = 90;

pub fn build(runtime: Rc<Runtime>, app: &adw::Application) -> adw::ApplicationWindow {
    style::install();

    let toast = adw::ToastOverlay::new();
    let state = State::new(runtime, toast.clone());

    let weak = Rc::downgrade(&state);
    let dispatch: Dispatch = Rc::new(move |action| {
        if let Some(state) = weak.upgrade() {
            state.handle(action);
        }
    });

    let caption_in = caption("PipewireEffects Sink");
    let list = crate::chain::new_list(&dispatch);
    let caption_out = caption("No output device");

    let add = gtk::Button::builder()
        .tooltip_text("Add an effect to the chain")
        .halign(gtk::Align::Fill)
        .build();
    let add_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    add_content.set_halign(gtk::Align::Center);
    add_content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    add_content.append(&gtk::Label::new(Some("Add effect")));
    add.set_child(Some(&add_content));
    add.add_css_class("pill");
    add.set_margin_top(8);
    add.set_margin_bottom(2);
    add.set_margin_start(10);
    add.set_margin_end(10);
    {
        let weak = Rc::downgrade(&state);
        add.connect_clicked(move |button| {
            let (Some(window), Some(state)) =
                (button.root().and_downcast::<gtk::Window>(), weak.upgrade())
            else {
                return;
            };
            crate::chain::present_add_dialog(&window, state.runtime.registry(), &state.dispatch());
        });
    }

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.append(&add);
    sidebar.append(&caption_in);
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();
    sidebar.append(&scroller);
    sidebar.append(&caption_out);

    let editor = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let editor_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&editor)
        .build();
    let empty = adw::StatusPage::builder()
        .icon_name("audio-x-generic-symbolic")
        .title("No effect selected")
        .description("Pick an effect on the left, or add one to get started.")
        .vexpand(true)
        .build();
    let stack = gtk::Stack::new();
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&editor_scroller, Some("effect"));
    stack.set_visible_child_name("empty");

    let paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .start_child(&sidebar)
        .end_child(&stack)
        .resize_start_child(false)
        .shrink_start_child(false)
        .shrink_end_child(false)
        .position(SIDEBAR_WIDTH)
        .build();
    paned.set_wide_handle(true);

    let (header, title, bypass) = build_header(&state);
    let (status_bar, widgets) = build_status_bar();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&paned));
    toolbar.add_bottom_bar(&status_bar);

    toast.set_child(Some(&toolbar));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("PipewireEffects")
        .content(&toast)
        .default_width(1_080)
        .default_height(740)
        .build();

    state.set_window(&window);
    *state.title.borrow_mut() = Some(title);
    *state.bypass.borrow_mut() = Some(bypass);
    *state.list.borrow_mut() = Some(list);
    *state.stack.borrow_mut() = Some(stack);
    *state.editor.borrow_mut() = Some(editor);
    *state.caption_out.borrow_mut() = Some(caption_out);
    *state.preset_button.borrow_mut() = Some(widgets.preset_button);
    *state.device_button.borrow_mut() = Some(widgets.device_button);
    *state.left_meter.borrow_mut() = Some(widgets.left_meter);
    *state.right_meter.borrow_mut() = Some(widgets.right_meter);
    *state.left_reading.borrow_mut() = Some(widgets.left_reading);
    *state.right_reading.borrow_mut() = Some(widgets.right_reading);
    *state.rate_reading.borrow_mut() = Some(widgets.rate_reading);
    *state.latency_reading.borrow_mut() = Some(widgets.latency_reading);
    *state.clip_warning.borrow_mut() = Some(widgets.clip_warning);

    state.set_dispatch(&dispatch);

    actions::install_actions(&window, &state, &dispatch);

    {
        let state = Rc::clone(&state);
        window.connect_close_request(move |_| {
            state.closing.set(true);
            gtk::glib::Propagation::Proceed
        });
    }

    state.refresh_chain();
    state.refresh_content();
    state.refresh_devices();
    state.refresh_presets();
    state.refresh_status();
    state.sync_bypass();
    state.update_title();
    state.begin();

    window
}

fn caption(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_margin_start(14);
    label.set_margin_end(14);
    label.set_margin_top(4);
    label.set_margin_bottom(4);
    style::add(&label, style::PIPELINE_CAPTION);
    label
}

fn status_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    style::add(&label, style::STATUS_LABEL);
    label
}

pub fn status_value(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    style::add(&label, style::STATUS_VALUE);
    style::add(&label, style::NUMERIC);
    label
}

type Header = (adw::HeaderBar, adw::WindowTitle, gtk::ToggleButton);

pub fn build_header(state: &Rc<State>) -> Header {
    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("PipewireEffects", "Audio effects for PipeWire");
    header.set_title_widget(Some(&title));

    let menu = gio::Menu::new();
    menu.append(Some("Refresh devices"), Some("win.refresh-devices"));
    menu.append(Some("Restart engine"), Some("win.restart-engine"));
    menu.append(Some("Keyboard shortcuts"), Some("win.shortcuts"));
    menu.append(Some("About PipewireEffects"), Some("win.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Main menu")
        .build();
    header.pack_end(&menu_button);

    let bypass = gtk::ToggleButton::builder()
        .active(true)
        .tooltip_text("Turn all effects on or off")
        .build();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name("system-shutdown-symbolic"));
    content.append(&gtk::Label::new(Some("Effects")));
    bypass.set_child(Some(&content));
    bypass.update_property(&[gtk::accessible::Property::Label("Toggle all effects")]);
    {
        let weak = Rc::downgrade(state);
        bypass.connect_toggled(move |button| {
            if let Some(state) = weak.upgrade() {
                state.handle(UiAction::SetBypassed(!button.is_active()));
            }
        });
    }
    header.pack_end(&bypass);

    (header, title, bypass)
}

pub struct StatusWidgets {
    preset_button: gtk::MenuButton,
    device_button: gtk::MenuButton,
    left_meter: Rc<Meter>,
    right_meter: Rc<Meter>,
    left_reading: gtk::Label,
    right_reading: gtk::Label,
    rate_reading: gtk::Label,
    latency_reading: gtk::Label,
    clip_warning: gtk::Image,
}

pub fn build_status_bar() -> (gtk::Box, StatusWidgets) {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    style::add(&bar, style::STATUS_BAR);

    let preset_button = gtk::MenuButton::builder()
        .label("Presets")
        .icon_name("user-bookmarks-symbolic")
        .tooltip_text("Load or save a preset")
        .build();
    preset_button.add_css_class("flat");

    let device_button = gtk::MenuButton::builder()
        .label("Output")
        .icon_name("audio-speakers-symbolic")
        .tooltip_text("Choose where audio is played")
        .build();
    device_button.add_css_class("flat");

    bar.append(&preset_button);
    bar.append(&device_button);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bar.append(&spacer);

    let clip_warning = gtk::Image::from_icon_name("dialog-warning-symbolic");
    clip_warning.set_tooltip_text(Some("The output is clipping"));
    clip_warning.add_css_class(style::ERRORS);
    clip_warning.set_visible(false);
    bar.append(&clip_warning);

    let rate_reading = status_value("—");
    bar.append(&status_label("Rate"));
    bar.append(&rate_reading);

    let latency_reading = status_value("—");
    bar.append(&status_label("Latency"));
    bar.append(&latency_reading);

    let left_meter = Meter::new(METER_WIDTH);
    let right_meter = Meter::new(METER_WIDTH);
    let left_reading = status_value("—");
    let right_reading = status_value("—");

    bar.append(&status_label("Out L"));
    bar.append(left_meter.widget());
    bar.append(&left_reading);
    bar.append(&status_label("R"));
    bar.append(right_meter.widget());
    bar.append(&right_reading);

    (
        bar,
        StatusWidgets {
            preset_button,
            device_button,
            left_meter,
            right_meter,
            left_reading,
            right_reading,
            rate_reading,
            latency_reading,
            clip_warning,
        },
    )
}
