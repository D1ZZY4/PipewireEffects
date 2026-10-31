use super::*;

#[test]
fn the_index_round_trips() {
    for kind in ALL {
        assert_eq!(PipelineKind::from_index(kind.index()), kind);
    }
}

#[test]
fn an_unknown_index_is_the_output_pipeline() {
    assert_eq!(PipelineKind::from_index(7), PipelineKind::Output);
}
