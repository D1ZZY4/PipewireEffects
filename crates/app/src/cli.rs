use std::process::ExitCode;

use pwe_core::i18n::{t, tr};
use pwe_engine::list_devices;
use pwe_presets::PresetStore;
use pwe_ui::Options;

pub enum Action {
    Help,
    Version,
    ListPresets,
    ListDevices,
    Run(Options),
}

pub fn parse(args: Vec<String>) -> Result<Action, String> {
    let mut options = Options::default();
    let mut arguments = args.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(Action::Help),
            "-V" | "--version" => return Ok(Action::Version),
            "--list-presets" => return Ok(Action::ListPresets),
            "--list-devices" => return Ok(Action::ListDevices),
            "--no-window" => options.show_window = false,
            "--preset" => {
                options.preset = Some(require(&mut arguments, "--preset")?);
            }
            "--device" => {
                options.device = Some(require(&mut arguments, "--device")?);
            }
            other if other.starts_with("--preset=") => {
                options.preset = Some(value_of(other));
            }
            other if other.starts_with("--device=") => {
                options.device = Some(value_of(other));
            }
            other if other.starts_with('-') => {
                return Err(tr("unknown option '{0}'; try --help", &[other]));
            }
            _ => {}
        }
    }
    Ok(Action::Run(options))
}

pub fn run(action: Action) -> ExitCode {
    match action {
        Action::Help => {
            print!("{}", help());
            ExitCode::SUCCESS
        }
        Action::Version => {
            println!("pipewire-effects {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Action::ListPresets => list_presets(),
        Action::ListDevices => list_devices_command(),
        Action::Run(options) => pwe_ui::run(options),
    }
}

pub fn help() -> String {
    format!(
        "PipeWire Effects {version}\n\
         {about}\n\
         \n\
         Usage: pipewire-effects [options]\n\
         \n\
         Options:\n\
         \x20 -h, --help           {help}\n\
         \x20 -V, --version        {version_option}\n\
         \x20     --preset NAME    {preset}\n\
         \x20     --device NAME    {device}\n\
         \x20     --no-window      {no_window}\n\
         \x20     --list-presets   {list_presets}\n\
         \x20     --list-devices   {list_devices}\n",
        version = env!("CARGO_PKG_VERSION"),
        about =
            t("Process application audio through a chain of effects on a virtual PipeWire sink."),
        help = t("Show this help and exit"),
        version_option = t("Show the version and exit"),
        preset = t("Load a preset on start"),
        device = t("Send the processed audio to this node"),
        no_window = t("Start without a window; the interface opens on the next launch"),
        list_presets = t("Print the available presets and exit"),
        list_devices = t("Print the PipeWire devices and exit"),
    )
}

fn list_presets() -> ExitCode {
    let (presets, warnings) = PresetStore::from_environment().list();
    for preset in presets {
        let origin = if preset.builtin {
            t("built in")
        } else if preset.deletable() {
            t("user")
        } else {
            t("system")
        };
        println!("{}\t{origin}", preset.name);
    }
    for warning in warnings {
        eprintln!("{warning}");
    }
    ExitCode::SUCCESS
}

fn list_devices_command() -> ExitCode {
    let devices = match list_devices() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!(
                "{}",
                tr(
                    "the audio server could not be reached: {0}",
                    &[&error.to_string()]
                )
            );
            return ExitCode::FAILURE;
        }
    };
    for device in &devices {
        let kind = if device.is_sink() {
            t("sink")
        } else if device.is_source() {
            t("source")
        } else {
            t("device")
        };
        println!("{}\t{kind}\t{}", device.name, device.description);
    }
    ExitCode::SUCCESS
}

fn require(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| tr("{0} needs a value", &[option]))
}

fn value_of(argument: &str) -> String {
    argument
        .split_once('=')
        .map(|(_, value)| value.to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(arguments: &[&str]) -> Result<Action, String> {
        parse(arguments.iter().map(|value| (*value).to_owned()).collect())
    }

    #[test]
    fn no_arguments_open_a_window() {
        match parse_str(&[]) {
            Ok(Action::Run(options)) => {
                assert!(options.show_window);
                assert!(options.preset.is_none());
            }
            _ => panic!("the empty command line should start the interface"),
        }
    }

    #[test]
    fn the_flags_are_read_in_both_spellings() {
        match parse_str(&["--preset", "Podcast", "--device=usb", "--no-window"]) {
            Ok(Action::Run(options)) => {
                assert_eq!(options.preset.as_deref(), Some("Podcast"));
                assert_eq!(options.device.as_deref(), Some("usb"));
                assert!(!options.show_window);
            }
            _ => panic!("the flags should be accepted"),
        }
        match parse_str(&["--preset=Podcast"]) {
            Ok(Action::Run(options)) => assert_eq!(options.preset.as_deref(), Some("Podcast")),
            _ => panic!("the equals spelling should be accepted"),
        }
    }

    #[test]
    fn an_unknown_option_is_rejected() {
        assert!(parse_str(&["--nope"]).is_err());
        assert!(parse_str(&["--preset"]).is_err());
    }

    #[test]
    fn the_help_text_lists_every_option() {
        let text = help();
        for option in [
            "--help",
            "--version",
            "--preset",
            "--device",
            "--no-window",
            "--list-presets",
            "--list-devices",
        ] {
            assert!(text.contains(option), "{option} is missing from the help");
        }
    }
}
