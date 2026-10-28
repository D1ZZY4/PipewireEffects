use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use pipewire_effects_dsp::registry;
use pipewire_effects_engine::{EngineConfig, list_devices};

#[derive(Debug, Parser)]
#[command(
    name = "pipewire-effects",
    version,
    about = "Audio effects for PipeWire",
    long_about = "Runs a virtual sink that applications can play into, processes the audio \
                  with a chain of effects, and plays the result on an output device.\n\n\
                  Make the virtual sink the default with:\n  \
                  pactl set-default-sink pipewire-effects-sink",
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(short, long, value_name = "NAME")]
    device: Option<String>,

    #[arg(
        short,
        long,
        value_name = "MS",
        default_value_t = 20,
        value_parser = clap::value_parser!(u64).range(1..=500)
    )]
    latency: u64,

    #[arg(long)]
    list_devices: bool,

    #[arg(long)]
    list_effects: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    if cli.list_effects {
        print_effects();
        return ExitCode::SUCCESS;
    }

    if cli.list_devices {
        return match print_devices() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("pipewire-effects: {message}");
                ExitCode::FAILURE
            }
        };
    }

    let config = EngineConfig {
        target_sink: cli.device.clone(),
        target_latency: Duration::from_millis(cli.latency),
        ..EngineConfig::default()
    };

    pipewire_effects_ui::run(registry(), config)
}

fn init_logging(verbosity: u8) {
    use tracing_subscriber::EnvFilter;

    let level = match verbosity {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "pipewire_effects={level},pipewire_effects_engine={level},pipewire_effects_ui={level}"
        ))
    });

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn print_effects() {
    let registry = registry();
    println!("{} effect(s) available:", registry.len());
    for category in pipewire_effects_core::EffectCategory::ALL {
        let in_category: Vec<_> = registry
            .iter()
            .filter(|definition| definition.category == category)
            .collect();
        if in_category.is_empty() {
            continue;
        }
        println!("\n{}", category.label());
        for definition in in_category {
            println!("  {:<18} {}", definition.kind.as_str(), definition.label);
            println!("  {:<18} {}", "", definition.summary);
            let probe = definition.instantiate();
            println!("  {:<18} {} parameter(s)", "", probe.parameters().len());
        }
    }
}

fn print_devices() -> Result<(), String> {
    let devices = list_devices(pipewire_effects_engine::DEFAULT_DISCOVERY_TIMEOUT)
        .map_err(|error| format!("could not enumerate devices: {error}"))?;

    if devices.is_empty() {
        println!("PipeWire reported no audio devices.");
        return Ok(());
    }

    println!("Output devices:");
    let sinks: Vec<_> = devices
        .iter()
        .filter(|device| device.is_sink() && !device.is_virtual_sink())
        .collect();
    if sinks.is_empty() {
        println!("  (none)");
    }
    for sink in sinks {
        println!("  {}", sink.display_name());
        println!("    node.name     {}", sink.name);
        println!("    media.class   {}", sink.media_class);
    }

    let sources: Vec<_> = devices
        .iter()
        .filter(|device| device.is_source() && !device.is_monitor())
        .collect();
    if !sources.is_empty() {
        println!("\nInput devices:");
        for source in sources {
            println!("  {}", source.display_name());
            println!("    node.name     {}", source.name);
        }
    }

    println!("\nPass an output device to `--device` to control where audio is played.");
    Ok(())
}
