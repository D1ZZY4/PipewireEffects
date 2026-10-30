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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_names_the_application_and_its_icon() {
        let entry = entry("pipewire-effects --no-window");
        assert!(entry.contains("Type=Application"));
        assert!(entry.contains("Exec=pipewire-effects --no-window"));
        assert!(entry.contains(&format!("Icon={APPLICATION_ID}")));
        assert!(entry.ends_with('\n'));
    }

    #[test]
    fn the_path_sits_below_the_autostart_directory() {
        let path = autostart_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("io.github.D1ZZY4.PipewireEffects.desktop")
        );
        assert!(
            path.parent()
                .is_some_and(|parent| parent.ends_with(AUTOSTART_DIR))
        );
    }
}
