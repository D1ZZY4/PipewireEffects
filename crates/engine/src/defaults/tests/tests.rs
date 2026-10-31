use super::*;

#[test]
fn the_sink_name_is_read_from_the_metadata_document() {
    assert_eq!(
        parse_sink("{\"name\":\"bluez_output.aa.1\"}").as_deref(),
        Some("bluez_output.aa.1")
    );
    assert_eq!(
        parse_sink("{ \"name\": \"speakers\" }").as_deref(),
        Some("speakers")
    );
}

#[test]
fn a_document_without_a_name_yields_nothing() {
    assert!(parse_sink("{}").is_none());
    assert!(parse_sink("{\"name\":\"\"}").is_none());
    assert!(parse_sink("not json").is_none());
}

#[test]
fn each_pipeline_watches_its_own_metadata_key() {
    assert_eq!(
        crate::config::metadata_key(PipelineKind::Output),
        "default.audio.sink"
    );
    assert_eq!(
        crate::config::metadata_key(PipelineKind::Input),
        "default.audio.source"
    );
}
