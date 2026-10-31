#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use pipewire as pw;

use crate::error::EngineResult;

pub fn roundtrip(
    main_loop: &pw::main_loop::MainLoopRc,
    core: &pw::core::CoreRc,
) -> EngineResult<()> {
    let done = Rc::new(Cell::new(false));
    let flag = Rc::clone(&done);
    let loop_clone = main_loop.clone();
    let pending = core.sync(0)?;
    let _listener = core
        .add_listener_local()
        .done(move |id, sequence| {
            if id == pw::core::PW_ID_CORE && sequence == pending {
                flag.set(true);
                loop_clone.quit();
            }
        })
        .register();
    while !done.get() {
        main_loop.run();
    }
    Ok(())
}

pub fn settle(main_loop: &pw::main_loop::MainLoopRc, iterations: usize, millis: u64) {
    for _ in 0..iterations {
        main_loop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(millis)));
    }
}
