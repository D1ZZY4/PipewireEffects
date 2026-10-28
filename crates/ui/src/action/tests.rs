use std::cell::RefCell;

use super::*;

#[test]
fn every_action_has_a_description() {
    let actions = [
        UiAction::Select(0),
        UiAction::SetEnabled(0, true),
        UiAction::SetEnabled(0, false),
        UiAction::MoveUp(0),
        UiAction::MoveDown(0),
        UiAction::Remove(0),
        UiAction::Add(String::from("gain")),
        UiAction::SetBypassed(true),
        UiAction::SetBypassed(false),
        UiAction::Clear,
        UiAction::ParametersChanged,
        UiAction::LoadPreset(String::from("x")),
        UiAction::SavePreset,
        UiAction::DeletePreset(String::from("x")),
        UiAction::SelectDevice(String::from("x")),
        UiAction::RefreshDevices,
        UiAction::RestartEngine,
    ];
    for action in actions {
        assert!(
            !action.description().is_empty(),
            "{action:?} has no description"
        );
    }
}

#[test]
fn dispatching_invokes_the_handler() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let handler = Rc::clone(&seen);
    let dispatch: Dispatch = Rc::new(move |action| handler.borrow_mut().push(action));
    dispatch(UiAction::Clear);
    assert_eq!(seen.borrow().len(), 1);
}
