use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};

use pipewire_effects_core::{ChainController, EffectDef};
use pipewire_effects_engine::{Engine, EngineConfig, EngineEvent, EngineHandles, EventSink};
use pipewire_effects_presets::PresetStore;

use crate::error::UiError;

pub struct Runtime {
    registry: &'static [EffectDef],
    controller: Rc<ChainController>,
    presets: PresetStore,
    config: RefCell<EngineConfig>,
    engine: RefCell<Option<Engine>>,
    handles: RefCell<Option<EngineHandles>>,
    events: RefCell<mpsc::Receiver<EngineEvent>>,
}

impl Runtime {
    pub fn new(registry: &'static [EffectDef], config: EngineConfig) -> Rc<Self> {
        let controller = Rc::new(ChainController::new(registry));
        controller.set_format(config.format);

        let runtime = Rc::new(Self {
            registry,
            controller,
            presets: discover_presets(),
            config: RefCell::new(config),
            engine: RefCell::new(None),
            handles: RefCell::new(None),
            events: RefCell::new(mpsc::channel().1),
        });

        let _ = runtime.start_engine();
        runtime
    }

    pub const fn registry(&self) -> &'static [EffectDef] {
        self.registry
    }

    pub fn controller(&self) -> &Rc<ChainController> {
        &self.controller
    }

    pub const fn presets(&self) -> &PresetStore {
        &self.presets
    }

    pub fn config(&self) -> EngineConfig {
        self.config.borrow().clone()
    }

    pub fn handles(&self) -> Option<EngineHandles> {
        self.handles.borrow().clone()
    }

    pub fn is_engine_running(&self) -> bool {
        self.engine
            .borrow()
            .as_ref()
            .is_some_and(Engine::is_running)
    }

    pub fn start_engine(&self) -> Result<(), UiError> {
        self.stop_engine();

        let (sender, receiver) = mpsc::channel();
        *self.events.borrow_mut() = receiver;

        let sender = Arc::new(Mutex::new(sender));
        let events: EventSink = Arc::new(move |event| {
            if let Ok(sender) = sender.lock() {
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

    pub fn stop_engine(&self) {
        if let Some(mut engine) = self.engine.borrow_mut().take() {
            engine.stop();
        }
        *self.handles.borrow_mut() = None;
    }

    pub fn restart_with(&self, mutate: impl FnOnce(&mut EngineConfig)) -> Result<(), UiError> {
        {
            let mut config = self.config.borrow_mut();
            mutate(&mut config);
            self.controller.set_format(config.format);
        }
        self.start_engine()
    }

    pub fn poll_events(&self) -> Vec<EngineEvent> {
        self.events.borrow().try_iter().collect()
    }
}

fn discover_presets() -> PresetStore {
    match PresetStore::discover() {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!("using a temporary preset directory: {error}");
            PresetStore::with_user_dir(std::env::temp_dir().join("pipewire-effects-presets"))
        }
    }
}
