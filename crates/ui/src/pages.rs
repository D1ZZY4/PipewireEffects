use std::rc::Rc;

use adw::prelude::*;
use pwe_core::i18n::{t, tr};
use pwe_core::{ALL_PIPELINES, PipelineKind};

use crate::chrome::{NARROW_LAYOUT, dialog, header_controls, install, install_dialog, main_menu};
use crate::devices::DevicesView;
use crate::format;
use crate::meter::LevelDisplay;
use crate::preferences::PreferencesView;
use crate::presets_ui;
use crate::widgets;

pub fn page_name(kind: PipelineKind) -> &'static str {
    match kind {
        PipelineKind::Output => "output",
        PipelineKind::Input => "input",
    }
}

pub fn pipeline_of(page: &str) -> Option<PipelineKind> {
    ALL_PIPELINES
        .into_iter()
        .find(|kind| page_name(*kind) == page)
}

#[derive(Clone)]
pub struct WorkspaceActions {
    pub on_target: Rc<dyn Fn(String)>,
    pub on_input: Rc<dyn Fn(bool)>,
    pub on_autostart: Rc<dyn Fn(bool)>,
    pub on_reconnect: Rc<dyn Fn()>,
    pub on_developer_mode: Rc<dyn Fn(bool)>,
}

pub struct ChainPane {
    pub kind: PipelineKind,
    pub list: gtk::ListBox,
    pub placeholder: gtk::Widget,
    pub editor_host: gtk::Box,
    pub add_button: gtk::Button,
    pub meter_in_box: gtk::Box,
    pub meter_in: LevelDisplay,
    pub meter_out_box: gtk::Box,
    pub meter_out: LevelDisplay,
}

pub struct Workspace {
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub banner: adw::Banner,
    pub stack: adw::ViewStack,
    pub panes: [ChainPane; 2],
    pub preset_list: gtk::ListBox,
    pub preset_buttons: (gtk::Button, gtk::Button, gtk::Button, gtk::Button),
    pub bypass: gtk::ToggleButton,
    pub devices_view: DevicesView,
    pub preferences_view: PreferencesView,
    pub devices_dialog: adw::PreferencesDialog,
    pub preferences_dialog: adw::PreferencesDialog,
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
        banner.set_button_label(Some(t("Dismiss")));
        banner.set_revealed(false);

        let panes = ALL_PIPELINES.map(chain_pane);
        let output_split = chain_page(&panes[0]);
        let input_split = chain_page(&panes[1]);

        let (presets_page, preset_list, preset_buttons) = presets_ui::build_page();
        let preset_scroll = gtk::ScrolledWindow::new();
        preset_scroll.set_vexpand(true);
        preset_scroll.set_child(Some(&presets_page));

        let (devices_page, devices_view) = DevicesView::new(actions.clone());
        let (preferences_page, preferences_view) = PreferencesView::new(actions);
        let devices_dialog = dialog(t("Devices"), &devices_page);
        let preferences_dialog = dialog(t("Preferences"), &preferences_page);

        let stack = adw::ViewStack::new();
        stack.add_titled_with_icon(
            &output_split,
            Some("output"),
            t("Output"),
            format::ICON_PAGE_OUTPUT,
        );
        stack.add_titled_with_icon(
            &input_split,
            Some("input"),
            t("Input"),
            format::ICON_PAGE_INPUT,
        );
        stack.add_titled_with_icon(
            &preset_scroll,
            Some("presets"),
            t("Presets"),
            format::ICON_PAGE_PRESETS,
        );

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        header.set_title_widget(Some(&switcher));

        let toggle = sidebar_toggle(&[output_split.clone(), input_split.clone()]);
        header.pack_start(&toggle);
        install_breakpoint(&window, &[output_split, input_split]);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banner);
        toolbar.set_content(Some(&stack));

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&toolbar));
        window.set_content(Some(&toast));
        install(app, &window, &stack);
        install_dialog(app, &window, "devices", &devices_dialog);
        install_dialog(app, &window, "preferences", &preferences_dialog);

        menu_button.set_menu_model(Some(&main_menu()));
        Self {
            window,
            toast,
            banner,
            stack,
            panes,
            preset_list,
            preset_buttons,
            bypass,
            devices_view,
            preferences_view,
            devices_dialog,
            preferences_dialog,
        }
    }
}

fn chain_pane(kind: PipelineKind) -> ChainPane {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("navigation-sidebar");
    list.set_valign(gtk::Align::Start);

    let editor_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    editor_host.set_vexpand(true);

    let add_button = gtk::Button::from_icon_name(format::ICON_ADD);
    add_button.add_css_class("flat");
    add_button.set_valign(gtk::Align::Center);
    add_button.set_tooltip_text(Some(t("Add an effect to the chain")));

    let (meter_in_box, meter_in) = LevelDisplay::new(t("In"), true);
    let (meter_out_box, meter_out) = LevelDisplay::new(t("Out"), true);

    ChainPane {
        kind,
        list,
        placeholder: crate::chrome::placeholder(t("Add an effect to start shaping your audio.")),
        editor_host,
        add_button,
        meter_in_box,
        meter_in,
        meter_out_box,
        meter_out,
    }
}

fn chain_page(pane: &ChainPane) -> adw::OverlaySplitView {
    let chain_scroll = gtk::ScrolledWindow::new();
    chain_scroll.set_vexpand(true);
    chain_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    chain_scroll.set_propagate_natural_width(false);
    chain_scroll.set_child(Some(&pane.list));

    let chain_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    chain_header.append(&widgets::section_title(&tr(
        "{0} chain",
        &[pane.kind.label()],
    )));
    let filler = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    filler.set_hexpand(true);
    chain_header.append(&filler);
    chain_header.append(&pane.add_button);

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.set_margin_top(10);
    sidebar.set_margin_bottom(10);
    sidebar.set_margin_start(10);
    sidebar.set_margin_end(10);
    sidebar.append(&chain_header);
    sidebar.append(&pane.placeholder);
    sidebar.append(&chain_scroll);

    let levels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    levels.append(&pane.meter_in_box);
    levels.append(&pane.meter_out_box);

    let editor_column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    editor_column.set_margin_top(10);
    editor_column.set_margin_bottom(10);
    editor_column.set_margin_start(14);
    editor_column.set_margin_end(14);
    editor_column.append(&levels);
    editor_column.append(&pane.editor_host);
    let editor_scroll = gtk::ScrolledWindow::new();
    editor_scroll.set_vexpand(true);
    editor_scroll.set_child(Some(&editor_column));

    adw::OverlaySplitView::builder()
        .sidebar(&sidebar)
        .content(&editor_scroll)
        .sidebar_position(gtk::PackType::Start)
        .min_sidebar_width(320.0)
        .max_sidebar_width(400.0)
        .build()
}

fn install_breakpoint(window: &adw::ApplicationWindow, splits: &[adw::OverlaySplitView]) {
    let Ok(condition) = adw::BreakpointCondition::parse(NARROW_LAYOUT) else {
        return;
    };
    let breakpoint = adw::Breakpoint::new(condition);
    for split in splits {
        breakpoint.add_setter(split, "collapsed", Some(&true.to_value()));
    }
    window.add_breakpoint(breakpoint);
}

fn sidebar_toggle(splits: &[adw::OverlaySplitView]) -> gtk::ToggleButton {
    let toggle = gtk::ToggleButton::new();
    toggle.set_icon_name(format::ICON_SIDEBAR);
    toggle.set_tooltip_text(Some(t("Show or hide the effect chain")));
    toggle.add_css_class("flat");
    toggle.set_visible(false);

    let view = splits.to_vec();
    toggle.connect_toggled(move |button| {
        for split in &view {
            split.set_show_sidebar(button.is_active());
        }
    });

    for split in splits {
        let button = toggle.clone();
        split.connect_show_sidebar_notify(move |view| {
            if button.is_active() != view.shows_sidebar() {
                button.set_active(view.shows_sidebar());
            }
        });
        let button = toggle.clone();
        split.connect_collapsed_notify(move |view| {
            button.set_visible(view.is_collapsed());
            if view.is_collapsed() {
                button.set_active(view.shows_sidebar());
            }
        });
    }
    toggle
}
