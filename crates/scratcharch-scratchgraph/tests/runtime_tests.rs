//! Tests for the ScratchGraph runtime abstraction.

use scratcharch_scratchgraph::ir::{EventHat, ScriptEntry, Stmt, Value as SgValue};
use scratcharch_scratchgraph::runtime::{
    EventState, ExecutionStatus, RuntimeState, SchedulerState, ThreadContext, ThreadId,
    WaitReason,
};

#[test]
fn test_thread_context_enter_pop_frame() {
    let mut ctx = ThreadContext::new();
    ctx.enter_frame(4);
    assert_eq!(ctx.frame_pointer, 0);
    assert_eq!(ctx.stack.len(), 4);
    assert_eq!(ctx.stack[0], 0.0); // saved FP

    ctx.frame_set(1, 42.0);
    assert_eq!(ctx.frame_get(1), Some(42.0));

    ctx.pop_frame(2); // pop locals only, leave saved FP + return slot
    assert_eq!(ctx.stack.len(), 2);
}

#[test]
fn test_thread_context_nested_frames() {
    let mut ctx = ThreadContext::new();
    ctx.enter_frame(3);
    ctx.frame_set(2, 7.0);

    ctx.enter_frame(3);
    assert!(ctx.frame_pointer > 0);
    ctx.frame_set(1, 99.0);
    assert_eq!(ctx.frame_get(1), Some(99.0));

    // Outer frame data remains intact at the old frame pointer.
    assert_eq!(ctx.stack.get(2), Some(&7.0));
}

#[test]
fn test_scheduler_spawn_and_enqueue() {
    let mut scheduler = SchedulerState::new();
    let entry = ScriptEntry::new(EventHat::GreenFlag, vec![]);
    let id = scheduler.spawn(entry);
    assert_eq!(id, ThreadId(0));

    scheduler.enqueue(id);
    assert_eq!(scheduler.runnable.len(), 1);
}

#[test]
fn test_event_state_registration() {
    let mut events = EventState::new();
    let green = ScriptEntry::new(EventHat::GreenFlag, vec![]);
    let key = ScriptEntry::new(EventHat::KeyPressed("space".to_string()), vec![]);

    events.register(ThreadId(0), &green.hat);
    events.register(ThreadId(1), &key.hat);

    assert_eq!(events.responders(&green.hat), vec![ThreadId(0)]);
    assert_eq!(events.responders(&key.hat), vec![ThreadId(1)]);
}

#[test]
fn test_runtime_dispatch_green_flag() {
    let mut runtime = RuntimeState::new();
    let entry = ScriptEntry::new(EventHat::GreenFlag, vec![]);
    runtime.register_project(vec![entry]);

    assert_eq!(runtime.scheduler.threads.len(), 1);
    runtime.dispatch_green_flag();
    assert_eq!(runtime.scheduler.runnable.len(), 1);
    assert_eq!(runtime.scheduler.threads[0].status, ExecutionStatus::Idle);
}

#[test]
fn test_runtime_dispatch_broadcast() {
    let mut runtime = RuntimeState::new();
    let green = ScriptEntry::new(EventHat::GreenFlag, vec![]);
    let recv = ScriptEntry::new(
        EventHat::BroadcastReceived("go".to_string()),
        vec![Stmt::Broadcast {
            message: scratcharch_scratchgraph::ir::Expr::Literal(SgValue::String("go".to_string())),
        }],
    );
    runtime.register_project(vec![green, recv.clone()]);

    runtime.dispatch_event(&recv.hat);
    assert_eq!(runtime.scheduler.runnable.len(), 1);
}

#[test]
fn test_scheduler_wait_and_stop() {
    let mut scheduler = SchedulerState::new();
    let id = scheduler.spawn(ScriptEntry::new(EventHat::GreenFlag, vec![]));
    scheduler.enqueue(id);
    let current = scheduler.schedule_next();
    assert_eq!(current, Some(id));

    scheduler.wait_current(WaitReason::Yield);
    assert_eq!(scheduler.current, None);
    assert_eq!(scheduler.waiting.len(), 1);

    // Re-enqueue the waiting thread and then stop it.
    scheduler.enqueue(id);
    scheduler.schedule_next();
    scheduler.stop_current();
    assert_eq!(scheduler.current, None);
    assert_eq!(scheduler.threads[id.0 as usize].status, ExecutionStatus::Stopped);
}
