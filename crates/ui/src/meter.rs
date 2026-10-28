use gtk::prelude::*;

use crate::format::{db_text, normalized_db};

pub struct LevelDisplay {
    left: gtk::LevelBar,
    right: gtk::LevelBar,
    label: gtk::Label,
}

impl LevelDisplay {
    pub fn new(title: &str) -> (gtk::Box, Self) {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
        container.add_css_class("meter-block");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let name = gtk::Label::new(Some(title));
        name.set_xalign(0.0);
        name.add_css_class("caption");
        name.set_hexpand(true);
        let label = gtk::Label::new(Some(&db_text(-120.0)));
        label.add_css_class("caption");
        label.add_css_class("numeric");
        header.append(&name);
        header.append(&label);

        let left = meter_bar();
        let right = meter_bar();
        container.append(&header);
        container.append(&left);
        container.append(&right);

        (container, Self { left, right, label })
    }

    pub fn update(&self, peak: [f32; 2], rms: [f32; 2]) {
        self.left.set_value(normalized_db(peak[0]));
        self.right.set_value(normalized_db(peak[1]));
        self.left.remove_offset_value(Some("rms"));
        self.right.remove_offset_value(Some("rms"));
        self.left.add_offset_value("rms", normalized_db(rms[0]));
        self.right.add_offset_value("rms", normalized_db(rms[1]));
        let loudest = peak[0].max(peak[1]);
        self.label.set_label(&db_text(loudest));
        if loudest > -0.5 {
            self.label.add_css_class("error");
        } else {
            self.label.remove_css_class("error");
        }
    }
}

fn meter_bar() -> gtk::LevelBar {
    let bar = gtk::LevelBar::new();
    bar.set_min_value(0.0);
    bar.set_max_value(1.0);
    bar.set_mode(gtk::LevelBarMode::Continuous);
    bar.set_value(0.0);
    bar.add_css_class("meter");
    bar.set_size_request(-1, 6);
    bar
}

#[cfg(test)]
mod tests {
    #[test]
    fn meter_levels_are_normalised_through_the_shared_helper() {
        assert!((crate::format::normalized_db(-6.0) - 0.9).abs() < 1.0e-6);
    }
}
