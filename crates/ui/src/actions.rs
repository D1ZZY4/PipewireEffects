use std::cell::RefCell;
use std::rc::Rc;

use crate::pages::WorkspaceActions;
use crate::state::State;

pub fn workspace_actions(state: &Rc<RefCell<State>>) -> WorkspaceActions {
    let on_target = {
        let state = Rc::clone(state);
        Rc::new(move |target: String| state.borrow_mut().set_target_device(target))
            as Rc<dyn Fn(String)>
    };
    let on_input = {
        let state = Rc::clone(state);
        Rc::new(move |enabled: bool| {
            let result = state.borrow_mut().set_input_enabled(enabled);
            if let Err(error) = result {
                state.borrow().warn(&error.to_string());
            }
        }) as Rc<dyn Fn(bool)>
    };
    let on_autostart = {
        let state = Rc::clone(state);
        Rc::new(move |enabled: bool| {
            let result = state.borrow_mut().set_autostart(enabled);
            if let Err(error) = result {
                state.borrow().warn(&error);
            }
        }) as Rc<dyn Fn(bool)>
    };
    let on_reconnect = {
        let state = Rc::clone(state);
        Rc::new(move || {
            let kind = state.borrow().active();
            let result = state.borrow_mut().restart(kind);
            if let Err(error) = result {
                state.borrow().warn(&error.to_string());
            }
        }) as Rc<dyn Fn()>
    };
    WorkspaceActions {
        on_target,
        on_input,
        on_autostart,
        on_reconnect,
    }
}
