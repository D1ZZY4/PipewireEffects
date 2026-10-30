use adw::prelude::*;

pub(super) fn scroll_to_bottom(widget: &gtk::Widget) -> bool {
    if let Some(scrolled) = widget.downcast_ref::<gtk::ScrolledWindow>() {
        let adjustment = scrolled.vadjustment();
        adjustment.set_value(adjustment.upper());
        return true;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if scroll_to_bottom(&current) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}
