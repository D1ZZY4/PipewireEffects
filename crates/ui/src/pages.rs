use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use pwe_core::i18n::t;
use pwe_core::{ALL_PIPELINES, PipelineKind};

use crate::format;
use crate::meter::LevelDisplay;
use crate::presets_ui;
use crate::settings::SettingsView;
use crate::widgets;

pub const PAGES: [(&str, &str, &str); 3] = [
    ("page-effects", "effects", "<primary>1"),
    ("page-presets", "presets", "<primary>2"),
    ("page-devices", "devices", "<primary>3"),
];

pub const NARROW_LAYOUT: &str = "max-width: 860px";

#[derive(Clone)]
pub struct WorkspaceActions {
    pub on_target: Rc<dyn Fn(String)>,
    pub on_input: Rc<dyn Fn(bool)>,
    pub on_autostart: Rc<dyn Fn(bool)>,
    pub on_reconnect: Rc<dyn Fn()>,
}

pub struct Workspace {
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub banner: adw::Banner,
    pub stack: adw::ViewStack,
    pub chain_list: gtk::ListBox,
    pub chain_placeholder: gtk::Widget,
    pub editor_host: gtk::Box,
    pub preset_list: gtk::ListBox,
    pub preset_buttons: (gtk::Button, gtk::Button, gtk::Button, gtk::Button),
    pub add_button: gtk::Button,
    pub bypass: gtk::ToggleButton,
    pub meter_in: LevelDisplay,
    pub meter_out: LevelDisplay,
    pub settings_view: SettingsView,
    pub pipeline_buttons: [gtk::ToggleButton; 2],
}

impl Workspace {
    pub fn build(app: &adw::Application, actions: WorkspaceActions) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("PipeWire Effects")
            .default_width(1040)
            .default_height(720)
            .build();

        let header = adw::HeaderBar::new();
        let (bypass, menu_button) = header_controls(&header);
        let banner = adw::Banner::new("");
        banner.set_revealed(false);

        let (input_box, meter_in) = LevelDisplay::new(t("Input"), true);
        let (output_box, meter_out) = LevelDisplay::new(t("Output"), true);

        let chain_list = gtk::ListBox::new();
        chain_list.set_selection_mode(gtk::SelectionMode::Single);
        chain_list.add_css_class("navigation-sidebar");
        chain_list.set_valign(gtk::Align::Start);

        let chain_placeholder = placeholder(t("Add an effect to start shaping your audio."));
        let chain_scroll = gtk::ScrolledWindow::new();
        chain_scroll.set_vexpand(true);
        chain_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        chain_scroll.set_propagate_natural_width(false);
        chain_scroll.set_child(Some(&chain_list));

        let add_button = gtk::Button::from_icon_name(format::ICON_ADD);
        add_button.add_css_class("flat");
        add_button.set_valign(gtk::Align::Center);
        add_button.set_tooltip_text(Some(t("Add an effect to the chain")));

        let (pipeline_row, pipeline_buttons) = pipeline_switcher();

        let chain_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chain_header.append(&widgets::section_title(t("Chain")));
        let filler = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        filler.set_hexpand(true);
        chain_header.append(&filler);
        chain_header.append(&add_button);

        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sidebar.set_margin_top(10);
        sidebar.set_margin_bottom(10);
        sidebar.set_margin_start(10);
        sidebar.set_margin_end(10);
        sidebar.append(&pipeline_row);
        sidebar.append(&chain_header);
        sidebar.append(&chain_placeholder);
        sidebar.append(&chain_scroll);

        let levels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        levels.append(&input_box);
        levels.append(&output_box);

        let editor_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_host.set_vexpand(true);
        let editor_column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        editor_column.set_margin_top(10);
        editor_column.set_margin_bottom(10);
        editor_column.set_margin_start(14);
        editor_column.set_margin_end(14);
        editor_column.append(&levels);
        editor_column.append(&editor_host);
        let editor_scroll = gtk::ScrolledWindow::new();
        editor_scroll.set_vexpand(true);
        editor_scroll.set_child(Some(&editor_column));

        let effects = effects_page(&window, &header, &sidebar, &editor_scroll);

        let (presets_page, preset_list, preset_buttons) = presets_ui::build_page();
        let preset_scroll = gtk::ScrolledWindow::new();
        preset_scroll.set_vexpand(true);
        preset_scroll.set_child(Some(&presets_page));

        let (settings_page, settings_view) = SettingsView::new(actions);
        let settings_scroll = gtk::ScrolledWindow::new();
        settings_scroll.set_vexpand(true);
        settings_scroll.set_child(Some(&settings_page));

        let stack = adw::ViewStack::new();
        stack.add_titled_with_icon(
            &effects,
            Some("effects"),
            t("Effects"),
            format::ICON_PAGE_EFFECTS,
        );
        stack.add_titled_with_icon(
            &preset_scroll,
            Some("presets"),
            t("Presets"),
            format::ICON_PAGE_PRESETS,
        );
        stack.add_titled_with_icon(
            &settings_scroll,
            Some("devices"),
            t("Devices"),
            format::ICON_PAGE_DEVICES,
        );

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        header.set_title_widget(Some(&switcher));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banner);
        toolbar.set_content(Some(&stack));

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&toolbar));
        window.set_content(Some(&toast));
        install(app, &window, &stack);

        menu_button.set_menu_model(Some(&main_menu()));
        Self {
            window,
            toast,
            banner,
            stack,
            chain_list,
            chain_placeholder,
            editor_host,
            preset_list,
            preset_buttons,
            add_button,
            bypass,
            meter_in,
            meter_out,
            settings_view,
            pipeline_buttons,
        }
    }
}

fn pipeline_switcher() -> (gtk::Box, [gtk::ToggleButton; 2]) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("linked");
    row.add_css_class("pipeline-switcher");
    row.set_halign(gtk::Align::Fill);
    row.set_homogeneous(true);

    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    for kind in ALL_PIPELINES {
        let button = gtk::ToggleButton::with_label(kind.label());
        button.add_css_class("flat");
        button.set_hexpand(true);
        button.set_tooltip_text(Some(match kind {
            PipelineKind::Output => t("Shape what applications play back"),
            PipelineKind::Input => t("Shape what your microphone captures"),
        }));
        if let Some(previous) = buttons.last() {
            button.set_group(Some(previous));
        }
        row.append(&button);
        buttons.push(button);
    }
    let buttons = match buttons.try_into() {
        Ok(array) => array,
        Err(_) => unreachable!("exactly two pipeline buttons are built"),
    };
    (row, buttons)
}

fn header_controls(header: &adw::HeaderBar) -> (gtk::ToggleButton, gtk::MenuButton) {
    let bypass = gtk::ToggleButton::new();
    bypass.set_icon_name(format::ICON_BYPASS);
    bypass.set_tooltip_text(Some(t("Bypass the effect chain")));
    bypass.update_property(&[
        gtk::accessible::Property::Label(t("Bypass the effect chain")),
        gtk::accessible::Property::Description(t(
            "Stops the audio from passing through the effects without removing them",
        )),
    ]);
    bypass.add_css_class("flat");
    bypass.add_css_class("bypass-button");
    header.pack_start(&bypass);

    let menu_button = gtk::MenuButton::builder()
        .icon_name(format::ICON_MENU)
        .tooltip_text(t("Main menu"))
        .build();
    header.pack_end(&menu_button);
    (bypass, menu_button)
}

fn main_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some(t("About PipeWire Effects")), Some("app.about"));
    menu.append(Some(t("Quit")), Some("app.quit"));
    menu
}

fn placeholder(text: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    label.set_margin_start(18);
    label.set_margin_end(18);
    label.set_margin_top(24);
    label.set_visible(false);
    label.upcast()
}

pub fn install(
    application: &adw::Application,
    window: &adw::ApplicationWindow,
    stack: &adw::ViewStack,
) {
    for (name, page, accelerator) in PAGES {
        let action = gio::SimpleAction::new(name, None);
        let stack = stack.clone();
        action.connect_activate(move |_, _| {
            stack.set_visible_child_name(page);
        });
        window.add_action(&action);
        application.set_accels_for_action(&format!("win.{name}"), &[accelerator]);
    }
}

pub fn effects_page(
    window: &adw::ApplicationWindow,
    header: &adw::HeaderBar,
    sidebar: &gtk::Box,
    content: &gtk::ScrolledWindow,
) -> adw::OverlaySplitView {
    let split = adw::OverlaySplitView::builder()
        .sidebar(sidebar)
        .content(content)
        .sidebar_position(gtk::PackType::Start)
        .min_sidebar_width(320.0)
        .max_sidebar_width(400.0)
        .build();

    let toggle = sidebar_toggle(&split);
    header.pack_start(&toggle);

    if let Ok(condition) = adw::BreakpointCondition::parse(NARROW_LAYOUT) {
        let breakpoint = adw::Breakpoint::new(condition);
        breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(breakpoint);
    }
    split
}

fn sidebar_toggle(split: &adw::OverlaySplitView) -> gtk::ToggleButton {
    let toggle = gtk::ToggleButton::new();
    toggle.set_icon_name(format::ICON_SIDEBAR);
    toggle.set_tooltip_text(Some(t("Show or hide the effect chain")));
    toggle.add_css_class("flat");
    toggle.set_visible(false);

    let view = split.clone();
    toggle.connect_toggled(move |button| view.set_show_sidebar(button.is_active()));

    let button = toggle.clone();
    split.connect_show_sidebar_notify(move |view| {
        if button.is_active() != view.shows_sidebar() {
            button.set_active(view.shows_sidebar());
        }
    });

    let button = toggle.clone();
    split.connect_collapsed_notify(move |view| button.set_visible(view.is_collapsed()));
    toggle
}
