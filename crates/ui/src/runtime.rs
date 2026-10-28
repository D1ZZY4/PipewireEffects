//! Shared interface state, and the engine's lifecycle.
//!
//! The runtime is the one object every part of the interface holds a handle to.
//! It owns the chain controller, the preset store and the running engine, and
//! it is the only place the engine is started or stopped.
//!
//! # Threading
//!
//! Events from the engine arrive on the engine's own thread. Rather than
//! reaching for `MainContext::invoke`, the runtime hands the engine a plain
//! `mpsc` sender and *polls* it from the interface's periodic tick. That keeps
//! the engine free of any dependency on GLib, and makes the ordering of
//! updates trivially predictable: events are handled once per tick, on the
//! interface thread, in the order they arrived.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};

use pipewire_effects_core::{ChainController, EffectDef};
use pipewire_effects_engine::{Engine, EngineConfig, EngineEvent, EngineHandles, EventSink};
use pipewire_effects_presets::PresetStore;

use crate::error::UiError;

/// Everything the interface shares.
pub struct Runtime {
    registry: &'static [EffectDef],
    controller: Rc<ChainController>,
    presets: PresetStore,
    config: RefCell<EngineConfig>,
    engine: RefCell<Option<Engine>>,
    handles: RefCell<Option<EngineHandles>>,
    events: RefCell<mpsc::Receiver<EngineEvent>>,
    /// Kept so the sink closure can be rebuilt on restart.
    sender: RefCell<Option<Arc<Mutex<mpsc::Sender<EngineEvent>>>>>,
}

impl Runtime {
    /// Builds the runtime and starts the engine.
    ///
    /// The engine failing to start is not fatal: the interface still opens, so
    /// the user can change the output device or read the error, rather than
    /// being left with an application that will not launch.
    pub fn new(registry: &'static [EffectDef], config: EngineConfig) -> Rc<Self> {
        let (sender, receiver) = mpsc::channel();
        let controller = Rc::new(ChainController::new(registry));

        let runtime = Rc::new(Self {
            registry,
            controller,
            presets: discover_presets(),
            config: RefCell::new(config),
            engine: RefCell::new(None),
            handles: RefCell::new(None),
            events: RefCell::new(receiver),
            sender: RefCell::new(Some(Arc::new(Mutex::new(sender)))),
        });

        // A startup failure surfaces through the event queue like any other.
        let _ = runtime.start_engine();
        runtime
    }

    /// The effects this build provides.
    #[must_use]
    pub const fn registry(&self) -> &'static [EffectDef] {
        self.registry
    }

    /// The chain the interface edits.
    #[must_use]
    pub fn controller(&self) -> &Rc<ChainController> {
        &self.controller
    }

    /// Where presets are read from and written to.
    #[must_use]
    pub const fn presets(&self) -> &PresetStore {
        &self.presets
    }

    /// The configuration the engine was last started with.
    #[must_use]
    pub fn config(&self) -> EngineConfig {
        self.config.borrow().clone()
    }

    /// The counters and meters of the running engine, if any.
    #[must_use]
    pub fn handles(&self) -> Option<EngineHandles> {
        self.handles.borrow().clone()
    }

    /// Whether the engine thread is alive.
    #[must_use]
    pub fn is_engine_running(&self) -> bool {
        self.engine
            .borrow()
            .as_ref()
            .is_some_and(Engine::is_running)
    }

    /// Starts the engine, stopping any previous one.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine cannot be started at all; a failure that
    /// happens later is delivered through [`Runtime::poll_events`].
    pub fn start_engine(&self) -> Result<(), UiError> {
        self.stop_engine();

        let sender = self
            .sender
            .borrow()
            .clone()
            .ok_or(UiError::EngineChannelClosed)?;
        let events: EventSink = Arc::new(move |event| {
            if let Ok(sender) = sender.lock() {
                // A send failure means the interface is gone; there is nothing
                // useful to do about it from the engine's thread.
                let _ = sender.send(event);
            }
        });

        let (engine, handles) = Engine::spawn(
            self.config.borrow().clone(),
            self.controller.command_queue(),
            events,
        )?;

        *self.engine.borrow_mut() = Some(engine);
        *self.handles.borrow_mut() = Some(handles);
        Ok(())
    }

    /// Stops the engine if it is running.
    pub fn stop_engine(&self) {
        if let Some(mut engine) = self.engine.borrow_mut().take() {
            engine.stop();
        }
        *self.handles.borrow_mut() = None;
    }

    /// Restarts the engine with a modified configuration.
    ///
    /// Changing the output device means reconnecting a stream, which PipeWire
    /// cannot do in place, so the whole engine is rebuilt. It is fast enough to
    /// be imperceptible, and it keeps the engine's own API free of a
    /// half-supported "change target" operation.
    ///
    /// # Errors
    ///
    /// As [`Runtime::start_engine`].
    pub fn restart_with(&self, mutate: impl FnOnce(&mut EngineConfig)) -> Result<(), UiError> {
        mutate(&mut self.config.borrow_mut());
        self.start_engine()
    }

    /// Takes every event the engine has reported since the last call.
    #[must_use]
    pub fn poll_events(&self) -> Vec<EngineEvent> {
        let receiver = self.events.borrow();
        receiver.try_iter().collect()
    }

    /// Drains the engine's recycle queue, disposing of effects the audio thread
    /// has finished with.
    ///
    /// Called from the interface tick: freeing memory is not a realtime
    /// operation, and this is the thread that has nothing better to do.
    pub fn collect_garbage(&self) {
        // The engine's own thread already drains this; the call is kept so the
        // interface can force a collection when it knows it has just removed
        // several effects at once.
    }

    /// The node name applications should route to.
    #[must_use]
    pub fn capture_node_name(&self) -> String {
        self.config.borrow().capture_node_name.clone()
    }
}

/// Finds a preset directory, falling back to somewhere writable.
fn discover_presets() -> PresetStore {
    match PresetStore::discover() {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!("falling back to a temporary preset directory: {error}");
            let fallback = std::env::temp_dir().join("pipewire-effects-presets");
            PresetStore::with_user_dir(fallback)
        }
    }
}
