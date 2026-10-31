use super::*;

#[test]
fn the_bundled_application_icon_resolves_in_the_icon_theme() {
    if gtk::init().is_err() {
        eprintln!("no display is available; the icon lookup test was not executed");
        return;
    }
    gio::resources_register_include!("app.gresource").expect("the resource should register");
    let Some(display) = gtk::gdk::Display::default() else {
        eprintln!("no display is available; the icon lookup test was not executed");
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    theme.add_resource_path(ICON_RESOURCE_PATH);
    assert!(
        theme.has_icon(APPLICATION_ID),
        "{APPLICATION_ID} was not found"
    );
    let missing = crate::format::missing_icons(&theme);
    assert!(missing.is_empty(), "the icon theme is missing {missing:?}");
}

#[test]
fn the_default_options_open_a_window() {
    assert!(Options::default().show_window);
    let headless = Options {
        show_window: false,
        ..Options::default()
    };
    assert!(!headless.show_window);
}
