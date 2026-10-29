use std::path::{Path, PathBuf};
use std::process::Command;

const RESOURCE_XML: &str = "io.github.D1ZZY4.PipewireEffects.gresource.xml";

fn main() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let xml = data.join(RESOURCE_XML);
    let icons = data.join("icons");
    let out_dir = std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR for build scripts");
    let target = PathBuf::from(out_dir).join("app.gresource");

    println!("cargo:rerun-if-changed={}", xml.display());
    for entry in resource_files(&xml) {
        println!("cargo:rerun-if-changed={}", icons.join(entry).display());
    }

    let result = Command::new("glib-compile-resources")
        .arg(&xml)
        .arg(format!("--sourcedir={}", icons.display()))
        .arg(format!("--target={}", target.display()))
        .arg("--generate")
        .output();

    match result {
        Ok(output) if output.status.success() => {}
        Ok(output) => panic!(
            "glib-compile-resources failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) => panic!("glib-compile-resources could not be run: {error}"),
    }
}

fn resource_files(xml: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(xml) else {
        return Vec::new();
    };
    text.split("<file>")
        .skip(1)
        .filter_map(|chunk| chunk.split("</file>").next())
        .map(|entry| entry.trim().to_owned())
        .filter(|entry| !entry.is_empty())
        .collect()
}
