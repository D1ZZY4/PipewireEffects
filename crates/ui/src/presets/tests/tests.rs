use super::*;

fn warnings(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("warning {index}")).collect()
}

#[test]
fn a_short_warning_list_is_shown_in_full() {
    assert_eq!(summarized(&warnings(2)), "warning 0, warning 1");
}

#[test]
fn a_long_warning_list_is_summarized_with_a_count() {
    assert_eq!(
        summarized(&warnings(5)),
        "warning 0, warning 1, warning 2 and 2 more"
    );
}

#[test]
fn a_clean_import_is_not_reported_as_a_warning() {
    assert_eq!(imported_banner("Podcast", &[]), "Imported 'Podcast'");
    assert_eq!(
        imported_banner("Podcast", &warnings(1)),
        "Imported 'Podcast' with warnings: warning 0"
    );
}
