use super::*;

#[test]
fn an_untranslated_message_is_returned_unchanged() {
    assert_eq!(t("Chain"), "Chain");
}

#[test]
fn repeated_lookups_are_stable_and_do_not_alias_a_reused_buffer() {
    let first = t("Chain");
    let second = t("Saved presets");
    let third = t("Chain");
    assert_eq!(first, "Chain");
    assert_eq!(second, "Saved presets");
    assert_eq!(third, "Chain");
    assert_eq!(tn("one", "many", 1), "one");
    assert_eq!(tn("one", "many", 3), "many");
}

#[test]
fn placeholders_are_substituted_in_order() {
    assert_eq!(tr("{0} and {1} more", &["one", "two"]), "one and two more");
}

#[test]
fn a_missing_placeholder_leaves_the_message_intact() {
    assert_eq!(tr("No placeholder", &["value"]), "No placeholder");
}

#[test]
fn plurals_fall_back_to_the_english_forms() {
    assert_eq!(tn("{0} more", "{0} more", 1), "{0} more");
    assert_eq!(tn("{0} more", "{0} more", 4), "{0} more");
}

#[test]
fn an_oversized_message_is_used_verbatim() {
    let long: &'static str = Box::leak("x".repeat(KEY_CAPACITY).into_boxed_str());
    assert_eq!(t(long), long);
}

#[test]
fn the_domain_and_the_directory_override_are_stable() {
    assert_eq!(DOMAIN, "pipewire-effects");
    assert!(!default_directory().as_os_str().is_empty());
}

#[test]
fn the_extraction_marker_is_the_identity() {
    assert_eq!(mark("Chain"), "Chain");
}
