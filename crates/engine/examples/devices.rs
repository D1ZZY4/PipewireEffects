fn main() {
    match pwe_engine::list_devices() {
        Ok(devices) => {
            println!("{} device nodes", devices.len());
            for device in devices {
                println!(
                    "  id={:<4} class={:<20} virtual={:<5} priority={:<6} name={:<32} description={}",
                    device.id,
                    device.media_class,
                    device.virtual_device,
                    device.priority,
                    device.name,
                    device.description
                );
            }
        }
        Err(error) => eprintln!("could not list devices: {error}"),
    }
}
