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
#[path = "tests/tests.rs"]
mod tests;
