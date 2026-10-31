use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use pwe_core::i18n::t;
use pwe_core::{ALL_PIPELINES, PipelineKind};

use crate::chrome::{
    dialog, effects_toggle, header_controls, install, install_breakpoint, install_dialog,
    main_menu, sidebar_toggle,
};
use crate::devices::DevicesView;
use crate::diagnostics::Diagnostics;
use crate::format;
use crate::meter::LevelDisplay;
use crate::preferences::PreferencesView;
use crate::presets_ui;
use crate::spectrum::SpectrumDisplay;

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
    pub on_auto_save: Rc<dyn Fn(bool)>,
    pub on_auto_save_interval: Rc<dyn Fn(u32)>,
    pub on_audio_backend: Rc<dyn Fn(String)>,
}

pub struct ChainPane {
    pub kind: PipelineKind,
    pub sidebar: gtk::Box,
    pub content: gtk::ScrolledWindow,
    pub list: gtk::ListBox,
    pub preset_list: gtk::ListBox,
    pub preset_buttons: (
        gtk::Button,
        gtk::Button,
        gtk::Button,
        gtk::Button,
        gtk::Button,
    ),
    pub placeholder: gtk::Widget,
    pub editor_host: gtk::Box,
    pub add_button: gtk::Button,
    pub effects: gtk::ToggleButton,
    pub close_button: gtk::Button,
    pub meter_in_box: gtk::Box,
    pub meter_in: LevelDisplay,
    pub meter_out_box: gtk::Box,
    pub meter_out: LevelDisplay,
    pub spectrum: SpectrumDisplay,
}

pub struct Workspace {
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub banner: adw::Banner,
    pub stack: adw::ViewStack,
    pub sidebar_stack: gtk::Stack,
    pub split: adw::OverlaySplitView,
    pub panes: [ChainPane; 2],
    pub devices_view: DevicesView,
    pub preferences_view: PreferencesView,
    pub diagnostics_view: Diagnostics,
    pub devices_dialog: adw::PreferencesDialog,
    pub preferences_dialog: adw::PreferencesDialog,
    pub diagnostics_dialog: adw::PreferencesDialog,
    pub diagnostics_action: gio::SimpleAction,
}

impl Workspace {
    pub fn build(app: &adw::Application, actions: WorkspaceActions) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("PipeWire Effects")
            .default_width(1040)
            .default_height(720)
            .build();
        window.set_size_request(360, 480);

        let header = adw::HeaderBar::new();
        let menu = header_controls(&header);
        let banner = adw::Banner::new("");
        banner.set_button_label(Some(t("Dismiss")));
        banner.set_revealed(false);

        let panes = ALL_PIPELINES.map(chain_pane);

        let (devices_page, devices_view) = DevicesView::new(actions.clone());
        let (preferences_page, preferences_view) = PreferencesView::new(actions);
        let diagnostics_view = Diagnostics::new();
        let devices_dialog = dialog(t("Devices"), &devices_page);
        let preferences_dialog = dialog(t("Preferences"), &preferences_page);
        let diagnostics_dialog = dialog(t("Diagnostics"), diagnostics_view.page());

        let stack = adw::ViewStack::new();
        let sidebar_stack = gtk::Stack::new();
        for pane in &panes {
            let name = page_name(pane.kind);
            stack.add_titled_with_icon(
                &pane.content,
                Some(name),
                t(pane.kind.label()),
                page_icon(pane.kind),
            );
            sidebar_stack.add_named(&pane.sidebar, Some(name));
        }

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        header.set_title_widget(Some(&switcher));

        let split = adw::OverlaySplitView::builder()
            .sidebar(&sidebar_stack)
            .sidebar_position(gtk::PackType::Start)
            .min_sidebar_width(300.0)
            .max_sidebar_width(380.0)
            .build();

        let toggle = sidebar_toggle(&split);
        header.pack_start(&toggle);
        install_breakpoint(&window, &split, &switcher);
        wire_close_buttons(&split, &panes);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banner);
        toolbar.set_content(Some(&stack));
        split.set_content(Some(&toolbar));

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&split));
        window.set_content(Some(&toast));
        install(app, &window, &stack);
        install_dialog(app, &window, "devices", &devices_dialog);
        install_dialog(app, &window, "preferences", &preferences_dialog);
        let diagnostics_action = install_dialog(app, &window, "diagnostics", &diagnostics_dialog);

        menu.set_menu_model(Some(&main_menu()));
        Self {
            window,
            toast,
            banner,
            stack,
            sidebar_stack,
            split,
            panes,
            devices_view,
            preferences_view,
            diagnostics_view,
            devices_dialog,
            preferences_dialog,
            diagnostics_dialog,
            diagnostics_action,
        }
    }
}

fn page_icon(kind: PipelineKind) -> &'static str {
    match kind {
        PipelineKind::Output => format::ICON_PAGE_OUTPUT,
        PipelineKind::Input => format::ICON_PAGE_INPUT,
    }
}

fn chain_pane(kind: PipelineKind) -> ChainPane {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("navigation-sidebar");
    list.set_valign(gtk::Align::Start);

    let preset_list = gtk::ListBox::new();
    preset_list.set_selection_mode(gtk::SelectionMode::None);
    preset_list.add_css_class("navigation-sidebar");
    preset_list.set_valign(gtk::Align::Start);

    let editor_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    editor_host.set_vexpand(true);

    let add_button = gtk::Button::from_icon_name(format::ICON_ADD);
    add_button.add_css_class("flat");
    add_button.set_valign(gtk::Align::Center);
    add_button.set_tooltip_text(Some(t("Add an effect to the chain")));

    let close_button = gtk::Button::from_icon_name(format::ICON_BACK);
    close_button.add_css_class("flat");
    close_button.set_valign(gtk::Align::Center);
    close_button.set_visible(false);
    close_button.set_tooltip_text(Some(t("Close the effect chain")));
    close_button.update_property(&[gtk::accessible::Property::Label(t(
        "Close the effect chain",
    ))]);

    let (preset_actions, preset_buttons) = presets_ui::actions_row();

    let (meter_in_box, meter_in) = LevelDisplay::new(t("In"), true);
    let (meter_out_box, meter_out) = LevelDisplay::new(t("Out"), true);
    let (spectrum_box, spectrum) = SpectrumDisplay::new();

    let placeholder = crate::chrome::placeholder(t("Add an effect to start shaping your audio."));
    let effects = effects_toggle();
    let sidebar = chain_sidebar(
        &placeholder,
        &list,
        &preset_list,
        &preset_actions,
        &effects,
        &close_button,
        &add_button,
    );
    let content = editor_column(&editor_host, (&meter_in_box, &meter_out_box), &spectrum_box);

    ChainPane {
        kind,
        sidebar,
        content,
        list,
        preset_list,
        preset_buttons,
        placeholder,
        editor_host,
        add_button,
        effects,
        close_button,
        meter_in_box,
        meter_in,
        meter_out_box,
        meter_out,
        spectrum,
    }
}

fn chain_sidebar(
    placeholder: &gtk::Widget,
    list: &gtk::ListBox,
    preset_list: &gtk::ListBox,
    preset_actions: &gtk::Box,
    effects: &gtk::ToggleButton,
    close_button: &gtk::Button,
    add_button: &gtk::Button,
) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.append(list);
    body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    body.append(preset_actions);
    body.append(preset_list);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_propagate_natural_width(false);
    scroll.set_child(Some(&body));

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let filler = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    filler.set_hexpand(true);
    header.append(effects);
    header.append(&filler);
    header.append(close_button);
    header.append(add_button);

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.set_margin_top(10);
    sidebar.set_margin_bottom(10);
    sidebar.set_margin_start(10);
    sidebar.set_margin_end(10);
    sidebar.append(&header);
    sidebar.append(&placeholder.clone());
    sidebar.append(&scroll);
    sidebar
}

fn editor_column(
    editor_host: &gtk::Box,
    meters: (&gtk::Box, &gtk::Box),
    spectrum: &gtk::Box,
) -> gtk::ScrolledWindow {
    let levels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    levels.append(meters.0);
    levels.append(meters.1);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_top(10);
    column.set_margin_bottom(10);
    column.set_margin_start(14);
    column.set_margin_end(14);
    column.append(&levels);
    column.append(spectrum);
    column.append(editor_host);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&column));
    scroll
}

fn wire_close_buttons(split: &adw::OverlaySplitView, panes: &[ChainPane; 2]) {
    for pane in panes {
        let split = split.clone();
        pane.close_button
            .connect_clicked(move |_| split.set_show_sidebar(false));
    }
    let buttons: Vec<gtk::Button> = panes.iter().map(|pane| pane.close_button.clone()).collect();
    let split = split.clone();
    split.connect_collapsed_notify(move |view| {
        let collapsed = view.is_collapsed();
        for button in &buttons {
            button.set_visible(collapsed);
        }
    });
}
