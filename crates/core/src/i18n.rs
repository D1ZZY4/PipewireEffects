use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};

pub const DOMAIN: &str = "pipewire-effects";
pub const DIRECTORY_VARIABLE: &str = "PIPEWIRE_EFFECTS_LOCALEDIR";

const KEY_CAPACITY: usize = 512;
const LC_ALL: std::ffi::c_int = 6;

unsafe extern "C" {
    fn gettext(msgid: *const c_char) -> *mut c_char;
    fn ngettext(singular: *const c_char, plural: *const c_char, count: u64) -> *mut c_char;
    fn bindtextdomain(domain: *const c_char, directory: *const c_char) -> *mut c_char;
    fn textdomain(domain: *const c_char) -> *mut c_char;
    fn setlocale(category: std::ffi::c_int, locale: *const c_char) -> *mut c_char;
}

pub fn default_directory() -> PathBuf {
    match std::env::var_os(DIRECTORY_VARIABLE) {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from("/usr/share/locale"),
    }
}

pub fn bind(directory: &Path) {
    let directory = key(directory.as_os_str().as_encoded_bytes());
    let domain = key(DOMAIN.as_bytes());
    unsafe {
        setlocale(LC_ALL, c"".as_ptr());
    }
    if let (Some(directory), Some(domain)) = (directory, domain) {
        unsafe {
            bindtextdomain(domain.as_ptr().cast(), directory.as_ptr().cast());
            textdomain(domain.as_ptr().cast());
        }
    }
    preload();
}

pub fn t(msgid: &'static str) -> &'static str {
    let Some(key) = key(msgid.as_bytes()) else {
        return msgid;
    };
    let message = key.as_ptr().cast();
    let pointer = unsafe { gettext(message) };
    if pointer.is_null() || std::ptr::eq(pointer.cast_const(), message) {
        return msgid;
    }
    unsafe { CStr::from_ptr(pointer) }.to_str().unwrap_or(msgid)
}

pub fn tn(singular: &'static str, plural: &'static str, count: u64) -> &'static str {
    let fallback = if count == 1 { singular } else { plural };
    let (Some(singular_key), Some(plural_key)) = (key(singular.as_bytes()), key(plural.as_bytes()))
    else {
        return fallback;
    };
    let singular_message = singular_key.as_ptr().cast();
    let plural_message = plural_key.as_ptr().cast();
    let pointer = unsafe { ngettext(singular_message, plural_message, count) };
    if pointer.is_null()
        || std::ptr::eq(pointer.cast_const(), singular_message)
        || std::ptr::eq(pointer.cast_const(), plural_message)
    {
        return fallback;
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .unwrap_or(fallback)
}

pub fn tr(msgid: &'static str, arguments: &[&str]) -> String {
    let mut text = t(msgid).to_owned();
    for (index, argument) in arguments.iter().enumerate() {
        text = text.replace(&format!("{{{index}}}"), argument);
    }
    text
}

pub const fn mark(msgid: &'static str) -> &'static str {
    msgid
}

fn key(bytes: &[u8]) -> Option<[u8; KEY_CAPACITY]> {
    if bytes.len() >= KEY_CAPACITY || bytes.contains(&0) {
        return None;
    }
    let mut buffer = [0u8; KEY_CAPACITY];
    buffer[..bytes.len()].copy_from_slice(bytes);
    Some(buffer)
}

fn preload() {
    let empty = [0u8; 1];
    let _ = unsafe { gettext(empty.as_ptr().cast()) };
}

#[cfg(test)]
mod tests {
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
}
