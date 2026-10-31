#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::fs;
use std::path::PathBuf;

use crate::error::SettingsResult;
use crate::store::config_home;

pub const APPLICATION_ID: &str = "io.github.D1ZZY4.PipewireEffects";
pub const AUTOSTART_DIR: &str = "autostart";
pub const DEFAULT_EXEC: &str = "pipewire-effects";

pub fn autostart_path() -> PathBuf {
    config_home()
        .join(AUTOSTART_DIR)
        .join(format!("{APPLICATION_ID}.desktop"))
}

pub fn entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=PipeWire Effects\n\
         Comment=Process application audio through the effect chain\n\
         Exec={exec}\n\
         Icon={APPLICATION_ID}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

pub fn set_enabled(enabled: bool, exec: &str) -> SettingsResult<()> {
    let path = autostart_path();
    if !enabled {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, entry(exec))?;
    Ok(())
}
