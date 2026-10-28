use adw::prelude::*;

const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+Q", "Quit"),
    ("Ctrl+W", "Close the window"),
    ("F11", "Full screen"),
    ("Right click a band", "Turn that equaliser band on or off"),
];

pub fn present(parent: &adw::ApplicationWindow) {
    let lines: Vec<String> = SHORTCUTS
        .iter()
        .map(|(keys, what)| format!("{keys}\t{what}"))
        .collect();

    let label = gtk::Label::new(Some(&lines.join("\n")));
    label.set_xalign(0.0);
    label.set_selectable(true);
    label.set_margin_top(12);
    label.set_margin_bottom(12);
    label.set_margin_start(12);
    label.set_margin_end(12);

    let dialog = adw::AlertDialog::new(Some("Keyboard shortcuts"), None);
    dialog.set_extra_child(Some(&label));
    dialog.add_response("close", "Close");
    dialog.set_close_response("close");
    dialog.present(Some(parent));
}
