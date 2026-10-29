use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pw::registry::Listener as RegistryListener;
use pwe_core::i18n::{t, tr};
use pwe_core::{ChainCommand, Effect};

use crate::command::{EffectSlot, EngineCommand};
use crate::config::{EngineConfig, PROCESS_TIMEOUT_MS, RETRY_BACKOFF_MS};
use crate::connection::{roundtrip, settle};
use crate::defaults::{self, Watch};
use crate::device::{self, DeviceInfo};
use crate::error::{EngineError, EngineResult};
use crate::listener;
use crate::shared::{EngineState, Shared};
use crate::stream_setup::{self, StreamSetup, Streams};
use crate::target;

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
    system_default: Rc<Watch>,
    retry_at: Option<Instant>,
    attempts: u32,
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
        let system_default = defaults::watch(config.owned_nodes().to_vec());
        let registry_listener =
            listener::install(&registry, &config, &shared, &devices, &system_default);

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
            system_default,
            retry_at: None,
            attempts: 0,
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
            self.follow_system_default();
            self.recover();
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
        self.release_streams();
        self.registry_listener.take();
        settle(&self.main_loop, 4, 10);
        self.drain_retired();
        self.shared.set_state(EngineState::Stopped);
        self.shared.update_snapshot(|snapshot| {
            snapshot.sample_rate = 0;
            snapshot.warnings.clear();
        });
    }

    fn release_streams(&mut self) {
        if let Some(streams) = self.streams.take() {
            let _ = streams.downstream.disconnect();
            let _ = streams.upstream.disconnect();
            streams.downstream_listener.unregister();
            streams.upstream_listener.unregister();
            settle(&self.main_loop, 4, 10);
        }
    }

    fn rebuild_streams(&mut self) -> EngineResult<()> {
        self.release_streams();
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

    fn recover(&mut self) {
        if self.shared.state() != EngineState::Failed {
            self.attempts = 0;
            self.retry_at = None;
            return;
        }
        let now = Instant::now();
        if self.retry_at.is_some_and(|at| now < at) {
            return;
        }
        self.attempts = self.attempts.saturating_add(1);
        let backoff = RETRY_BACKOFF_MS.saturating_mul(u64::from(self.attempts.min(8)));
        self.retry_at = Some(now + Duration::from_millis(backoff));
        self.shared.set_state(EngineState::Starting);
        self.shared.set_error(None);
        self.shared.add_warning(tr(
            "the {0} pipeline is reconnecting (attempt {1})",
            &[self.config.kind.noun(), &self.attempts.to_string()],
        ));
        match self.rebuild_streams() {
            Ok(()) => {
                self.attempts = 0;
                self.retry_at = None;
            }
            Err(error) => self.report(error),
        }
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
            EngineCommand::Stop => {}
        }
    }

    fn add_effect(&mut self, slot: EffectSlot, effect: Box<dyn Effect>) {
        if self.mirror.len() >= crate::config::MAX_EFFECTS {
            self.shared.add_warning(tr(
                "the chain is limited to {0} effects",
                &[&crate::config::MAX_EFFECTS.to_string()],
            ));
            let _ = self.retired.push(effect);
            return;
        }
        if self.commands.push(ChainCommand::Add(effect)).is_ok() {
            self.mirror.push(slot);
        } else {
            self.shared
                .add_warning(t("the audio thread command queue is full").to_owned());
        }
    }

    fn drain_retired(&self) {
        while let Some(effect) = self.retired.pop() {
            drop(effect);
        }
    }

    fn resolve_target(&mut self) {
        let devices = self
            .devices
            .lock()
            .map_or_else(|_| Vec::new(), |known| known.clone());
        let system_default = self.system_default.current();
        self.target = target::resolve(
            &self.shared,
            &devices,
            &self.config,
            system_default.as_deref(),
        );
    }

    fn follow_system_default(&mut self) {
        if !self.config.follows_the_system_default() {
            return;
        }
        let Some(wanted) = self.system_default.current() else {
            return;
        };
        let devices = self
            .devices
            .lock()
            .map_or_else(|_| Vec::new(), |known| known.clone());
        let own = self.config.own_nodes();
        match device::resolve(&devices, &wanted, &own, self.config.kind) {
            Some(chosen) if chosen.name != self.target => {
                self.target = chosen.name.clone();
                if let Err(error) = self.rebuild_streams() {
                    self.report(error);
                }
            }
            _ => {}
        }
    }

    fn report_devices(&self) {
        target::report(&self.shared, &self.devices);
    }

    fn report(&self, error: EngineError) {
        self.shared.add_warning(error.to_string());
    }

    fn refresh_snapshot(&self) {
        let rate = self.rate.load(Ordering::Relaxed);
        let config = self.config.clone();
        let target = self.target.clone();
        let live = self.streams.is_some();
        let node_id = self
            .streams
            .as_ref()
            .map_or(0, |streams| streams.virtual_node_id())
            .min(u32::MAX - 1);
        self.shared.update_snapshot(move |snapshot| {
            snapshot.kind = config.kind;
            snapshot.sample_rate = rate;
            snapshot.channels = config.channels;
            snapshot.target = target.clone();
            snapshot.bypassed = config.bypassed;
            snapshot.automatic = config.follows_the_system_default();
            snapshot.node_name = config.node_name.clone();
            snapshot.node_id = node_id;
            snapshot.node_description = config.node_description.clone();
            if !live {
                snapshot.sample_rate = 0;
            }
        });
    }
}
