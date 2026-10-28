use gtk::gdk;
use gtk::prelude::*;

pub const NUMERIC: &str = "numeric";
pub const EFFECT_ROW: &str = "effect-row";
pub const INACTIVE: &str = "inactive";
pub const PIPELINE_CAPTION: &str = "pipeline-caption";
pub const STATUS_BAR: &str = "status-bar";
pub const STATUS_LABEL: &str = "status-label";
pub const STATUS_VALUE: &str = "status-value";
pub const STATUS_HOT: &str = "status-hot";
pub const STATUS_CLIPPING: &str = "status-clipping";
pub const BAND_CELL: &str = "band-cell";
pub const BAND_CELL_ACTIVE: &str = "band-cell-active";
pub const BAND_STRIP: &str = "band-strip";
pub const ERRORS: &str = "errors";

const CSS: &str = r"
.numeric {
  font-feature-settings: 'tnum' 1;
}

.effect-row {
  padding: 6px 8px;
  border-radius: 8px;
}
.effect-row.inactive {
  opacity: 0.5;
}

.pipeline-caption {
  font-size: 0.8em;
  opacity: 0.6;
  padding: 6px 10px 2px 10px;
}
.pipeline-caption.bottom {
  padding: 2px 10px 6px 10px;
}

.status-bar {
  padding: 3px 12px;
}
.status-label {
  font-size: 0.85em;
  opacity: 0.6;
}
.status-value {
  font-size: 0.85em;
  font-feature-settings: 'tnum' 1;
  font-weight: 600;
}
.status-value.status-hot {
  color: @warning_color;
}
.status-value.status-clipping {
  color: @error_color;
}

.band-strip {
  padding: 6px 4px;
}
.band-cell {
  padding: 4px 2px;
  border-radius: 8px;
}
.band-cell.band-cell-active {
  background-color: alpha(@accent_bg_color, 0.28);
}
label.band-cell {
  font-size: 0.78em;
}

.errors {
  color: @error_color;
}
";

pub fn install() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    let Some(display) = gdk::Display::default() else {
        return;
    };
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

pub fn add(widget: &impl IsA<gtk::Widget>, class: &str) {
    widget.as_ref().add_css_class(class);
}

pub fn remove(widget: &impl IsA<gtk::Widget>, class: &str) {
    widget.as_ref().remove_css_class(class);
}

pub fn set_inactive(widget: &impl IsA<gtk::Widget>, inactive: bool) {
    let widget = widget.as_ref();
    if inactive {
        widget.add_css_class(INACTIVE);
    } else {
        widget.remove_css_class(INACTIVE);
    }
}
