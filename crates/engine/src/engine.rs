use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use crossbeam_queue::ArrayQueue;
use pwe_core::{ChainCommand, Effect};

use crate::command::{EffectSlot, EngineCommand};
use crate::config::{EngineConfig, MAX_EFFECTS};
use crate::error::{EngineError, EngineResult};
use crate::runtime::Runtime;
use crate::shared::{EngineState, Shared};

pub const COMMAND_QUEUE_CAPACITY: usize = 64;
pub const RETIRE_QUEUE_CAPACITY: usize = 32;

pub struct EngineHandle {
    config: EngineConfig,
    commands: Sender<EngineCommand>,
    shared: Arc<Shared>,
    retired: Arc<ArrayQueue<Box<dyn Effect>>>,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn spawn(config: EngineConfig) -> EngineResult<Self> {
        let shared = Arc::new(Shared::new());
        shared.set_state(EngineState::Starting);
        let (sender, receiver) = channel();
        let thread = match spawn_thread(&config, Arc::clone(&shared), receiver) {
            Ok(thread) => thread,
            Err(error) => {
                shared.set_error(Some(error.to_string()));
                shared.set_state(EngineState::Failed);
                return Err(error);
            }
        };
        let retired: Arc<ArrayQueue<Box<dyn Effect>>> =
            Arc::new(ArrayQueue::new(RETIRE_QUEUE_CAPACITY));
        Ok(Self {
            config,
            commands: sender,
            shared,
            retired,
            thread: Some(thread),
        })
    }

    pub fn kind(&self) -> pwe_core::PipelineKind {
        self.config.kind
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn max_effects(&self) -> usize {
        MAX_EFFECTS
    }

    pub fn awaiting_recycling(&self) -> usize {
        self.retired.len()
    }

    pub fn started(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn send(&self, command: EngineCommand) -> EngineResult<()> {
        self.commands
            .send(command)
            .map_err(|_| EngineError::NotConnected)
    }

    pub fn add_effect(&self, slot: EffectSlot, effect: Box<dyn Effect>) -> EngineResult<()> {
        self.send(EngineCommand::AddEffect { slot, effect })
    }

    pub fn move_effect(&self, slot: EffectSlot, position: usize) -> EngineResult<()> {
        self.send(EngineCommand::MoveEffect { slot, position })
    }

    pub fn remove_effect(&self, slot: EffectSlot) -> EngineResult<()> {
        self.send(EngineCommand::RemoveEffect { slot })
    }

    pub fn set_effect_enabled(&self, slot: EffectSlot, enabled: bool) -> EngineResult<()> {
        self.send(EngineCommand::SetEffectEnabled { slot, enabled })
    }

    pub fn set_bypass(&self, bypassed: bool) -> EngineResult<()> {
        self.send(EngineCommand::SetBypass(bypassed))
    }

    pub fn set_target_device(&self, target: impl Into<String>) -> EngineResult<()> {
        self.send(EngineCommand::SetTargetDevice(target.into()))
    }

    pub fn refresh_devices(&self) -> EngineResult<()> {
        self.send(EngineCommand::RefreshDevices)
    }

    pub fn restart(&mut self) -> EngineResult<()> {
        self.join();
        self.shared
            .update_snapshot(|snapshot| snapshot.warnings.clear());
        self.shared.set_error(None);
        self.shared.set_state(EngineState::Starting);
        let (sender, receiver) = channel();
        let thread = spawn_thread(&self.config, Arc::clone(&self.shared), receiver)?;
        self.commands = sender;
        self.thread = Some(thread);
        Ok(())
    }

    pub fn shutdown(&mut self) {
        let _ = self.commands.send(EngineCommand::Stop);
        self.join();
        self.shared.set_state(EngineState::Stopped);
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn spawn_thread(
    config: &EngineConfig,
    shared: Arc<Shared>,
    receiver: std::sync::mpsc::Receiver<EngineCommand>,
) -> EngineResult<JoinHandle<()>> {
    let commands: Arc<ArrayQueue<ChainCommand>> = Arc::new(ArrayQueue::new(COMMAND_QUEUE_CAPACITY));
    let retired: Arc<ArrayQueue<Box<dyn Effect>>> =
        Arc::new(ArrayQueue::new(RETIRE_QUEUE_CAPACITY));
    let config = config.clone();
    std::thread::Builder::new()
        .name(format!("pipewire-effects-{}", config.kind.noun()))
        .spawn(move || {
            match Runtime::start(
                config,
                Arc::clone(&shared),
                Arc::clone(&commands),
                Arc::clone(&retired),
            ) {
                Ok(mut runtime) => runtime.run(&receiver),
                Err(error) => {
                    shared.set_error(Some(error.to_string()));
                    shared.set_state(EngineState::Failed);
                }
            }
        })
        .map_err(|error| EngineError::ThreadSpawn(error.to_string()))
}
