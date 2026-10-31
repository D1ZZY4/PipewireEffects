#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

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
    let on_developer_mode = {
        let state = Rc::clone(state);
        Rc::new(move |enabled: bool| state.borrow_mut().set_developer_mode(enabled))
            as Rc<dyn Fn(bool)>
    };
    let on_auto_save = {
        let state = Rc::clone(state);
        Rc::new(move |enabled: bool| state.borrow_mut().set_auto_save(enabled)) as Rc<dyn Fn(bool)>
    };
    let on_auto_save_interval = {
        let state = Rc::clone(state);
        Rc::new(move |seconds: u32| state.borrow_mut().set_auto_save_interval(seconds))
            as Rc<dyn Fn(u32)>
    };
    let on_audio_backend = {
        let state = Rc::clone(state);
        Rc::new(move |value: String| state.borrow_mut().set_audio_backend(&value))
            as Rc<dyn Fn(String)>
    };
    let on_spectrum_range = {
        let state = Rc::clone(state);
        Rc::new(move |range: u8| state.borrow_mut().set_spectrum_range(range)) as Rc<dyn Fn(u8)>
    };
    let on_spectrum_shape = {
        let state = Rc::clone(state);
        Rc::new(move |shape: u8| state.borrow_mut().set_spectrum_shape(shape)) as Rc<dyn Fn(u8)>
    };
    let on_spectrum_points = {
        let state = Rc::clone(state);
        Rc::new(move |points: u8| state.borrow_mut().set_spectrum_points(points)) as Rc<dyn Fn(u8)>
    };
    let on_spectrum_height = {
        let state = Rc::clone(state);
        Rc::new(move |height: u8| state.borrow_mut().set_spectrum_height(height)) as Rc<dyn Fn(u8)>
    };
    let on_spectrum_frequency_range = {
        let state = Rc::clone(state);
        Rc::new(move |range: u8| state.borrow_mut().set_spectrum_frequency_range(range))
            as Rc<dyn Fn(u8)>
    };
    let on_spectrum_fft = {
        let state = Rc::clone(state);
        Rc::new(move |fft: u8| state.borrow_mut().set_spectrum_fft(fft)) as Rc<dyn Fn(u8)>
    };
    let on_spectrum_window = {
        let state = Rc::clone(state);
        Rc::new(move |window: u8| state.borrow_mut().set_spectrum_window(window)) as Rc<dyn Fn(u8)>
    };
    let on_spectrum_auto_range = {
        let state = Rc::clone(state);
        Rc::new(move |enabled: bool| state.borrow_mut().set_spectrum_auto_range(enabled))
            as Rc<dyn Fn(bool)>
    };
    let on_autoload_remember = {
        let state = Rc::clone(state);
        Rc::new(move || {
            let mut guard = state.borrow_mut();
            let device = guard.snapshot_of(guard.active()).target;
            let preset = guard.loaded_preset().map(str::to_owned);
            if let Some(preset) = preset {
                guard.remember_autoload(&device, &preset);
            }
        }) as Rc<dyn Fn()>
    };
    let on_autoload_forget = {
        let state = Rc::clone(state);
        Rc::new(move || {
            let mut guard = state.borrow_mut();
            let device = guard.snapshot_of(guard.active()).target;
            guard.forget_autoload(&device);
        }) as Rc<dyn Fn()>
    };
    WorkspaceActions {
        on_target,
        on_input,
        on_autostart,
        on_reconnect,
        on_developer_mode,
        on_auto_save,
        on_auto_save_interval,
        on_audio_backend,
        on_spectrum_range,
        on_spectrum_shape,
        on_spectrum_points,
        on_spectrum_height,
        on_spectrum_frequency_range,
        on_spectrum_fft,
        on_spectrum_window,
        on_spectrum_auto_range,
        on_autoload_remember,
        on_autoload_forget,
    }
}
