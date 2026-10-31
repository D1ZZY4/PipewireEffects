use crate::cli::{Action, help, parse};

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
