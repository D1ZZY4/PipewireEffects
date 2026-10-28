use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pw::registry::Listener as RegistryListener;
use pwe_core::{ChainCommand, Effect};

use crate::command::{EffectSlot, EngineCommand};
use crate::config::{EngineConfig, PROCESS_TIMEOUT_MS};
use crate::device::{self, DeviceInfo};
use crate::error::{EngineError, EngineResult};
use crate::shared::{EngineState, Shared};
use crate::stream_setup::{self, StreamSetup, Streams};

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

pub struct Runtime {
    main_loop: pw::main_loop::MainLoopRc,
    core: pw::core::CoreRc,
    _registry: pw::registry::RegistryRc,
    shared: Arc<Shared>,
    config: EngineConfig,
    rate: Arc<AtomicU32>,
    commands: Arc<ArrayQueue<ChainCommand>>,
    retired: Arc<ArrayQueue<Box<dyn Effect>>>,
    devices: Arc<Mutex<Vec<DeviceInfo>>>,
    streams: Option<Streams>,
    registry_listener: Option<RegistryListener>,
    mirror: Vec<EffectSlot>,
    target: String,
}

impl Runtime {
    pub fn start(
        config: EngineConfig,
        shared: Arc<Shared>,
        commands: Arc<ArrayQueue<ChainCommand>>,
        retired: Arc<ArrayQueue<Box<dyn Effect>>>,
    ) -> EngineResult<Self> {
        pw::init();
        let main_loop = pw::main_loop::MainLoopRc::new(None)?;
        let context = pw::context::ContextRc::new(&main_loop, None)?;
        let core = context.connect_rc(None)?;
        let registry = core.get_registry_rc()?;

        let devices: Arc<Mutex<Vec<DeviceInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let collected = Arc::clone(&devices);
        let shared_for_listener = Arc::clone(&shared);
        let registry_listener = registry
            .add_listener_local()
            .global(move |object| {
                if object.type_ != pw::types::ObjectType::Node {
                    return;
                }
                let Some(props) = object.props else {
                    return;
                };
                let Some(info) = DeviceInfo::from_properties(object.id, props) else {
                    return;
                };
                if let Ok(mut known) = collected.lock()
                    && !known.iter().any(|existing| existing.name == info.name)
                {
                    known.push(info);
                    shared_for_listener
                        .update_snapshot(|snapshot| snapshot.devices = known.clone());
                }
            })
            .register();

        let mut runtime = Self {
            main_loop,
            core,
            _registry: registry,
            shared,
            rate: Arc::new(AtomicU32::new(48_000)),
            config,
            commands,
            retired,
            devices,
            streams: None,
            registry_listener: Some(registry_listener),
            mirror: Vec::new(),
            target: String::new(),
        };
        roundtrip(&runtime.main_loop, &runtime.core)?;
        runtime.report_devices();
        runtime.resolve_target();
        runtime.rebuild_streams()?;
        Ok(runtime)
    }

    pub fn run(&mut self, receiver: &Receiver<EngineCommand>) {
        loop {
            while let Ok(command) = receiver.try_recv() {
                if matches!(command, EngineCommand::Stop) {
                    self.shutdown();
                    return;
                }
                self.handle(command);
            }
            self.drain_retired();
            self.refresh_snapshot();
            self.main_loop
                .loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(
                    PROCESS_TIMEOUT_MS,
                )));
            if self.shared.state() == EngineState::Stopped {
                return;
            }
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(streams) = self.streams.take() {
            let _ = streams.playback.disconnect();
            let _ = streams.capture.disconnect();
            streams.playback_listener.unregister();
            streams.capture_listener.unregister();
        }
        self.registry_listener.take();
        self.settle();
        self.drain_retired();
        self.shared.set_state(EngineState::Stopped);
        self.shared.update_snapshot(|snapshot| {
            snapshot.sample_rate = 0;
            snapshot.warnings.clear();
        });
    }

    fn settle(&self) {
        for _ in 0..4 {
            self.main_loop
                .loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(10)));
        }
    }

    fn rebuild_streams(&mut self) -> EngineResult<()> {
        if let Some(streams) = self.streams.take() {
            let _ = streams.playback.disconnect();
            let _ = streams.capture.disconnect();
            streams.playback_listener.unregister();
            streams.capture_listener.unregister();
            self.settle();
        }
        let setup = StreamSetup {
            core: self.core.clone(),
            config: self.config.clone(),
            shared: Arc::clone(&self.shared),
            rate: Arc::clone(&self.rate),
            target: self.target.clone(),
            bypassed: self.config.bypassed,
            commands: Arc::clone(&self.commands),
            retired: Arc::clone(&self.retired),
        };
        let streams = stream_setup::build(setup)?;
        self.streams = Some(streams);
        self.shared.set_state(EngineState::Running);
        Ok(())
    }

    fn handle(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::SetTargetDevice(target) => {
                self.config.target = target;
                self.resolve_target();
                if let Err(error) = self.rebuild_streams() {
                    self.report(error);
                }
            }
            EngineCommand::SetBypass(state) => {
                self.config.bypassed = state;
                let _ = self.commands.push(ChainCommand::SetBypass(state));
                self.shared
                    .update_snapshot(|snapshot| snapshot.bypassed = state);
            }
            EngineCommand::AddEffect { slot, effect } => self.add_effect(slot, effect),
            EngineCommand::SetEffectEnabled { slot, enabled } => {
                if let Some(index) = self.mirror.iter().position(|candidate| *candidate == slot) {
                    let _ = self
                        .commands
                        .push(ChainCommand::SetEnabled { index, enabled });
                }
            }
            EngineCommand::RemoveEffect { slot } => {
                if let Some(index) = self.mirror.iter().position(|candidate| *candidate == slot)
                    && self.commands.push(ChainCommand::Remove(index)).is_ok()
                {
                    self.mirror.remove(index);
                }
            }
            EngineCommand::MoveEffect { slot, position } => {
                if let Some(index) = self.mirror.iter().position(|candidate| *candidate == slot) {
                    let destination = position.min(self.mirror.len().saturating_sub(1));
                    if index != destination
                        && self
                            .commands
                            .push(ChainCommand::Move {
                                from: index,
                                to: destination,
                            })
                            .is_ok()
                    {
                        let moved = self.mirror.remove(index);
                        self.mirror.insert(destination, moved);
                    }
                }
            }
            EngineCommand::ResetChain => {
                for index in (0..self.mirror.len()).rev() {
                    let _ = self.commands.push(ChainCommand::Remove(index));
                }
                self.mirror.clear();
            }
            EngineCommand::RefreshDevices => {
                self.report_devices();
            }
            EngineCommand::Stop => {}
        }
    }

    fn add_effect(&mut self, slot: EffectSlot, effect: Box<dyn Effect>) {
        if self.mirror.len() >= crate::config::MAX_EFFECTS {
            self.shared.add_warning(format!(
                "the chain is limited to {} effects",
                crate::config::MAX_EFFECTS
            ));
            let _ = self.retired.push(effect);
            return;
        }
        if self.commands.push(ChainCommand::Add(effect)).is_ok() {
            self.mirror.push(slot);
        } else {
            self.shared
                .add_warning("the audio thread command queue is full".to_owned());
        }
    }

    fn drain_retired(&self) {
        while let Some(effect) = self.retired.pop() {
            drop(effect);
        }
    }

    fn resolve_target(&mut self) {
        let sink_name = self.config.sink_node_name.clone();
        let devices = self
            .devices
            .lock()
            .map_or_else(|_| Vec::new(), |known| known.clone());
        match device::resolve(&devices, &self.config.target, &sink_name) {
            Some(device) => self.target = device.name.clone(),
            None => {
                if !self.config.target.is_empty() && self.config.target != "default" {
                    self.shared
                        .add_warning(format!("no output device matches '{}'", self.config.target));
                }
                self.target = String::new();
            }
        }
    }

    fn report_devices(&self) {
        let devices = self
            .devices
            .lock()
            .map_or_else(|_| Vec::new(), |known| known.clone());
        self.shared
            .update_snapshot(move |snapshot| snapshot.devices = devices);
    }

    fn report(&self, error: EngineError) {
        self.shared.add_warning(error.to_string());
    }

    fn refresh_snapshot(&self) {
        let rate = self.rate.load(Ordering::Relaxed);
        let config = self.config.clone();
        let target = self.target.clone();
        let live = self.streams.is_some();
        self.shared.update_snapshot(move |snapshot| {
            snapshot.sample_rate = rate;
            snapshot.channels = config.channels;
            snapshot.target = if target.is_empty() {
                "no destination".to_owned()
            } else {
                target.clone()
            };
            snapshot.bypassed = config.bypassed;
            snapshot.sink_node_name = config.sink_node_name.clone();
            snapshot.sink_description = config.sink_description.clone();
            if !live {
                snapshot.sample_rate = 0;
            }
        });
    }
}
