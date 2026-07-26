//! ScratchGraph runtime abstraction.
//!
//! This module models the execution state of a Scratch VM without implementing
//! a full interpreter. It provides the IR-level primitives needed to reason
//! about green-flag startup, event dispatch, script lifecycle, cooperative
//! scheduling, and per-thread call stacks.
//!
//! All Scratch-specific runtime concepts stay inside
//! `scratcharch-scratchgraph`; no crate below it knows about threads,
//! event hats, or the Scratch scheduler.

use std::collections::HashMap;

use crate::ir::{EventHat, ScriptEntry, Value};

/// Unique identifier for a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadId(pub u64);

/// Execution status of a script thread.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    /// The thread exists but is not running.
    Idle,
    /// The thread is actively executing.
    Running,
    /// The thread has yielded and will resume later.
    Waiting(WaitReason),
    /// The thread has terminated.
    Stopped,
}

/// Reason a thread is waiting.
#[derive(Debug, Clone, PartialEq)]
pub enum WaitReason {
    /// Waiting for a timer to expire.
    Timer { remaining_seconds: f64 },
    /// Waiting until a condition becomes true.
    Until { condition: String },
    /// Waiting for broadcast responders to finish.
    BroadcastAndWait { message: String },
    /// Waiting for the next scheduler frame (general yield).
    Yield,
}

/// Program counter within a script body.
///
/// Scratch scripts are flat stacks of statements, so a PC is simply an index
/// into the body vector. Nested control structures are expanded into separate
/// statement sequences by the lowerer/exporter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProgramCounter {
    pub statement_index: usize,
}

/// Per-thread call-stack context.
///
/// v0.3 used a single global `__scratcharch_stack` and `__scratcharch_fp`,
/// which made concurrent scripts unsafe. v0.4 introduces `ThreadContext` so
/// each script thread owns its own stack storage. The generated Scratch code
/// still uses the same list/variable names at runtime; the scheduler is
/// responsible for switching the active context.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThreadContext {
    /// Per-thread call stack. In generated Scratch this becomes a list.
    pub stack: Vec<f64>,
    /// 0-based frame pointer into `stack`.
    pub frame_pointer: usize,
}

impl ThreadContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push `slots` cells onto the stack and update the frame pointer.
    /// The first pushed cell becomes the saved caller FP.
    pub fn enter_frame(&mut self, slots: u32) {
        let saved_fp = self.frame_pointer as f64;
        self.stack.push(saved_fp);
        for _ in 1..slots {
            self.stack.push(0.0);
        }
        self.frame_pointer = self.stack.len() - slots as usize;
    }

    /// Pop `slots` cells from the end of the stack.
    pub fn pop_frame(&mut self, slots: u32) {
        let slots = slots as usize;
        if self.stack.len() >= slots {
            self.stack.truncate(self.stack.len() - slots);
        } else {
            self.stack.clear();
        }
    }

    /// Read a frame-relative slot.
    pub fn frame_get(&self, offset: u32) -> Option<f64> {
        self.stack.get(self.frame_pointer + offset as usize).copied()
    }

    /// Write a frame-relative slot.
    pub fn frame_set(&mut self, offset: u32, value: f64) {
        let idx = self.frame_pointer + offset as usize;
        if idx < self.stack.len() {
            self.stack[idx] = value;
        }
    }
}

/// State of a single script thread.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadState {
    pub id: ThreadId,
    pub entry: ScriptEntry,
    pub context: ThreadContext,
    pub status: ExecutionStatus,
    pub pc: ProgramCounter,
}

impl ThreadState {
    pub fn new(id: ThreadId, entry: ScriptEntry) -> Self {
        Self {
            id,
            entry,
            context: ThreadContext::new(),
            status: ExecutionStatus::Idle,
            pc: ProgramCounter::default(),
        }
    }

    pub fn with_status(mut self, status: ExecutionStatus) -> Self {
        self.status = status;
        self
    }
}

/// Dispatch table from event hats to the threads that respond to them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventState {
    /// Threads waiting for the green flag.
    pub green_flag: Vec<ThreadId>,
    /// Threads waiting for a specific key press.
    pub key_pressed: HashMap<String, Vec<ThreadId>>,
    /// Threads waiting for a sprite or stage click.
    pub sprite_clicked: Vec<ThreadId>,
    /// Threads waiting for a named broadcast.
    pub broadcast_received: HashMap<String, Vec<ThreadId>>,
    /// Threads waiting for clone start.
    pub clone_start: Vec<ThreadId>,
}

impl EventState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a thread as responding to a particular event hat.
    pub fn register(&mut self, thread_id: ThreadId, hat: &EventHat) {
        match hat {
            EventHat::GreenFlag => self.green_flag.push(thread_id),
            EventHat::KeyPressed(key) => {
                self.key_pressed.entry(key.clone()).or_default().push(thread_id);
            }
            EventHat::SpriteClicked => self.sprite_clicked.push(thread_id),
            EventHat::BroadcastReceived(name) => {
                self.broadcast_received
                    .entry(name.clone())
                    .or_default()
                    .push(thread_id);
            }
            EventHat::CloneStart => self.clone_start.push(thread_id),
        }
    }

    /// Return the thread IDs that respond to the given event.
    pub fn responders(&self, hat: &EventHat) -> Vec<ThreadId> {
        match hat {
            EventHat::GreenFlag => self.green_flag.clone(),
            EventHat::KeyPressed(key) => self.key_pressed.get(key).cloned().unwrap_or_default(),
            EventHat::SpriteClicked => self.sprite_clicked.clone(),
            EventHat::BroadcastReceived(name) => self
                .broadcast_received
                .get(name)
                .cloned()
                .unwrap_or_default(),
            EventHat::CloneStart => self.clone_start.clone(),
        }
    }
}

/// Cooperative scheduler state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SchedulerState {
    /// All known threads, indexed by their `ThreadId.0` value for O(1) lookup.
    pub threads: Vec<ThreadState>,
    /// Queue of runnable thread IDs.
    pub runnable: Vec<ThreadId>,
    /// Threads that have yielded and are waiting.
    pub waiting: Vec<ThreadId>,
    /// Currently running thread, if any.
    pub current: Option<ThreadId>,
    /// Monotonically increasing counter for thread IDs.
    next_thread_id: u64,
}

impl SchedulerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new idle thread and return its ID.
    pub fn spawn(&mut self, entry: ScriptEntry) -> ThreadId {
        let id = ThreadId(self.next_thread_id);
        self.next_thread_id += 1;
        self.threads.push(ThreadState::new(id, entry));
        id
    }

    /// Mark a thread as runnable.
    pub fn enqueue(&mut self, id: ThreadId) {
        if !self.runnable.contains(&id) {
            self.runnable.push(id);
        }
    }

    /// Pick the next runnable thread without executing it.
    pub fn schedule_next(&mut self) -> Option<ThreadId> {
        let id = self.runnable.remove(0);
        self.current = Some(id);
        Some(id)
    }

    /// Move the current thread to the waiting set.
    pub fn wait_current(&mut self, reason: WaitReason) {
        if let Some(id) = self.current.take() {
            if let Some(thread) = self.thread_mut(id) {
                thread.status = ExecutionStatus::Waiting(reason);
            }
            self.waiting.push(id);
        }
    }

    /// Stop the current thread.
    pub fn stop_current(&mut self) {
        if let Some(id) = self.current.take() {
            if let Some(thread) = self.thread_mut(id) {
                thread.status = ExecutionStatus::Stopped;
            }
        }
    }

    /// Lookup a thread by ID.
    pub fn thread(&self, id: ThreadId) -> Option<&ThreadState> {
        self.threads.get(id.0 as usize)
    }

    /// Mutable lookup a thread by ID.
    pub fn thread_mut(&mut self, id: ThreadId) -> Option<&mut ThreadState> {
        self.threads.get_mut(id.0 as usize)
    }
}

/// Global runtime state shared by all threads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlobalState {
    /// Global and sprite-local variable values.
    pub variables: HashMap<String, Value>,
    /// Global and sprite-local list values.
    pub lists: HashMap<String, Vec<f64>>,
    /// Broadcast messages that have been sent this frame.
    pub broadcasts: Vec<String>,
}

impl GlobalState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Top-level runtime state for a Scratch project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuntimeState {
    /// Cooperative scheduler and thread table.
    pub scheduler: SchedulerState,
    /// Event dispatch table.
    pub events: EventState,
    /// Shared variables, lists, and broadcasts.
    pub globals: GlobalState,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register all threads from a project and build the event dispatch table.
    ///
    /// This is an IR-level abstraction: it does not execute scripts.
    pub fn register_project(&mut self, entries: Vec<ScriptEntry>) {
        for entry in entries {
            let id = self.scheduler.spawn(entry.clone());
            self.events.register(id, &entry.hat);
        }
    }

    /// Dispatch a green-flag event: stop other scripts and enqueue green-flag threads.
    pub fn dispatch_green_flag(&mut self) {
        for thread in &mut self.scheduler.threads {
            thread.status = ExecutionStatus::Idle;
            thread.context = ThreadContext::new();
            thread.pc = ProgramCounter::default();
        }
        self.scheduler.runnable.clear();
        self.scheduler.waiting.clear();
        self.scheduler.current = None;

        for id in &self.events.green_flag {
            self.scheduler.enqueue(*id);
        }
    }

    /// Dispatch an event to all matching threads.
    pub fn dispatch_event(&mut self, hat: &EventHat) {
        for id in self.events.responders(hat) {
            if let Some(thread) = self.scheduler.thread_mut(id) {
                thread.status = ExecutionStatus::Idle;
                thread.pc = ProgramCounter::default();
            }
            self.scheduler.enqueue(id);
        }
    }
}
