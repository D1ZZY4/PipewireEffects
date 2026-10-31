use super::*;

fn pair(name: &str, description: &str) -> (String, String) {
    (name.to_owned(), description.to_owned())
}

#[test]
fn unique_descriptions_are_used_as_labels() {
    let labels = device_labels(&[
        pair("alsa_output.a", "Built-in Audio"),
        pair("bluez.1", "Headset"),
    ]);
    assert_eq!(labels, vec!["Built-in Audio", "Headset"]);
}

#[test]
fn repeated_descriptions_are_numbered() {
    let labels = device_labels(&[
        pair("alsa_output.a", "Built-in Audio Pro"),
        pair("alsa_output.b", "Built-in Audio Pro"),
    ]);
    assert_eq!(
        labels,
        vec!["Built-in Audio Pro (1)", "Built-in Audio Pro (2)"]
    );
}

#[test]
fn a_missing_description_falls_back_to_the_name() {
    let labels = device_labels(&[pair("node.one", "")]);
    assert_eq!(labels, vec!["node.one"]);
}
