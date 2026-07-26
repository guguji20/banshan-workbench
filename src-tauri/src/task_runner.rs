use crate::protocol::{HostError, TaskDomainEvent, TaskEventType, TaskRecord, TaskStatus};
use crate::task_engine::{TaskEngine, TaskLifecycleOutcome};
use serde::Serialize;
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

const STARTUP_RECOVERY_TRACE: &str = "task-runner:startup-recovery";
const SHUTDOWN_RECOVERY_TRACE: &str = "task-runner:shutdown-recovery";
const FALLBACK_WAKE_INTERVAL: Duration = Duration::from_secs(5);
const UNREGISTERED_HANDLER_BACKOFF: Duration = Duration::from_secs(5);
pub const MIN_WORKER_COUNT: usize = 2;
pub const MAX_WORKER_COUNT: usize = 8;

pub type TaskHandlerResult = Result<Value, TaskHandlerError>;
pub type TaskLifecycleEventSink =
    Arc<dyn Fn(TaskDomainEvent) -> Result<(), HostError> + Send + Sync + 'static>;

/// A durable task handler. Implementations must observe the cancellation state around lengthy
/// or irreversible work and should use `report_progress` for committed progress updates.
pub trait TaskHandler: Send + Sync + 'static {
    fn execute(&self, context: HandlerContext) -> TaskHandlerResult;
}

impl<F> TaskHandler for F
where
    F: Fn(HandlerContext) -> TaskHandlerResult + Send + Sync + 'static,
{
    fn execute(&self, context: HandlerContext) -> TaskHandlerResult {
        self(context)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskHandlerError {
    code: String,
    message: String,
    retryable: bool,
}

impl TaskHandlerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self::structured("TASK_HANDLER_FAILED", message, false)
    }

    pub fn structured(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        let code = code.into();
        let code = code.trim();
        let message = message.into();
        let message = message.trim();
        Self {
            code: if code.is_empty() {
                "TASK_HANDLER_FAILED".to_string()
            } else {
                code.chars().take(120).collect()
            },
            message: if message.is_empty() {
                "task handler failed without an error message".to_string()
            } else {
                message.chars().take(16_000).collect()
            },
            retryable,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }

    fn encoded(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"code":"TASK_HANDLER_FAILED","message":"handler failed","retryable":false}"#
                .to_string()
        })
    }
}

impl fmt::Display for TaskHandlerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TaskHandlerError {}

impl From<HostError> for TaskHandlerError {
    fn from(error: HostError) -> Self {
        Self::structured(error.code, error.message, error.retryable)
    }
}

impl From<String> for TaskHandlerError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for TaskHandlerError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

#[derive(Clone)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

struct CancellationState {
    canceled: AtomicBool,
    wait_lock: Mutex<()>,
    wait_signal: Condvar,
}

impl CancellationToken {
    fn new() -> Self {
        Self {
            state: Arc::new(CancellationState {
                canceled: AtomicBool::new(false),
                wait_lock: Mutex::new(()),
                wait_signal: Condvar::new(),
            }),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.canceled.load(Ordering::Acquire)
    }

    pub fn wait_cancelled(&self, timeout: Duration) -> bool {
        if self.is_cancelled() {
            return true;
        }
        let guard = lock_unpoisoned(&self.state.wait_lock);
        drop(
            self.state
                .wait_signal
                .wait_timeout_while(guard, timeout, |_| !self.is_cancelled())
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        self.is_cancelled()
    }

    fn cancel(&self) {
        let _guard = lock_unpoisoned(&self.state.wait_lock);
        if !self.state.canceled.swap(true, Ordering::AcqRel) {
            self.state.wait_signal.notify_all();
        }
    }
}

#[derive(Clone)]
pub struct ProgressReporter {
    engine: Arc<TaskEngine>,
    task: String,
    attempt: u32,
    cancel: CancellationToken,
    event_sink: Arc<RwLock<Option<TaskLifecycleEventSink>>>,
    trace_id: String,
}

impl ProgressReporter {
    pub fn report(&self, progress: u8) -> Result<TaskRecord, HostError> {
        if self.cancel.is_cancelled() {
            return Err(HostError::new(
                "TASK_EXECUTION_CANCELED",
                format!("task {} is no longer the active attempt", self.task),
                false,
            ));
        }
        let outcome = self.engine.update_progress_with_events(
            &self.task,
            self.attempt,
            progress,
            &self.trace_id,
        )?;
        forward_events(&self.event_sink, outcome.emitted_events);
        Ok(outcome.task)
    }
}

/// The only capabilities visible to handlers: identity, immutable input/project context,
/// cooperative cancellation, and durable progress reporting.
pub struct HandlerContext {
    pub task: String,
    pub attempt: u32,
    pub input: Value,
    pub project: Option<String>,
    pub cancel: CancellationToken,
    pub progress: ProgressReporter,
}

struct WakeState {
    started: bool,
    shutdown: bool,
    generation: u64,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct AttemptKey {
    task_id: String,
    attempt: u32,
}

struct RunnerShared {
    engine: Arc<TaskEngine>,
    handlers: RwLock<HashMap<String, Arc<dyn TaskHandler>>>,
    event_sink: Arc<RwLock<Option<TaskLifecycleEventSink>>>,
    running: Mutex<HashMap<AttemptKey, CancellationToken>>,
    claims_deferred_until: Mutex<Option<Instant>>,
    wake_state: Mutex<WakeState>,
    wake_signal: Condvar,
}

/// Event-driven fixed-size worker pool over the authoritative SQLite `TaskEngine`.
pub struct TaskRunner {
    shared: Arc<RunnerShared>,
    worker_count: usize,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

impl TaskRunner {
    pub fn new(engine: Arc<TaskEngine>, worker_count: usize) -> Result<Self, HostError> {
        if !(MIN_WORKER_COUNT..=MAX_WORKER_COUNT).contains(&worker_count) {
            return Err(HostError::validation(format!(
                "task runner worker count must be in {MIN_WORKER_COUNT}..={MAX_WORKER_COUNT}"
            )));
        }
        Ok(Self {
            shared: Arc::new(RunnerShared {
                engine,
                handlers: RwLock::new(HashMap::new()),
                event_sink: Arc::new(RwLock::new(None)),
                running: Mutex::new(HashMap::new()),
                claims_deferred_until: Mutex::new(None),
                wake_state: Mutex::new(WakeState {
                    started: false,
                    shutdown: false,
                    generation: 0,
                }),
                wake_signal: Condvar::new(),
            }),
            worker_count,
            workers: Mutex::new(Vec::new()),
        })
    }

    pub fn with_event_sink(
        engine: Arc<TaskEngine>,
        worker_count: usize,
        event_sink: TaskLifecycleEventSink,
    ) -> Result<Self, HostError> {
        let runner = Self::new(engine, worker_count)?;
        runner.set_event_sink(Some(event_sink));
        Ok(runner)
    }

    pub fn engine(&self) -> Arc<TaskEngine> {
        Arc::clone(&self.shared.engine)
    }

    pub fn set_event_sink(&self, event_sink: Option<TaskLifecycleEventSink>) {
        *write_unpoisoned(&self.shared.event_sink) = event_sink;
    }

    /// Registers or replaces a handler. Returns true when an existing handler was replaced.
    pub fn register_handler<H>(
        &self,
        kind: impl Into<String>,
        handler: H,
    ) -> Result<bool, HostError>
    where
        H: TaskHandler,
    {
        let kind = kind.into();
        let kind = kind.trim();
        if kind.is_empty() || kind.chars().count() > 120 {
            return Err(HostError::validation(
                "task handler kind length must be 1..120",
            ));
        }
        if lock_unpoisoned(&self.shared.wake_state).shutdown {
            return Err(HostError::new(
                "TASK_RUNNER_SHUT_DOWN",
                "task runner has already shut down",
                false,
            ));
        }
        let replaced = write_unpoisoned(&self.shared.handlers)
            .insert(kind.to_string(), Arc::new(handler))
            .is_some();
        *lock_unpoisoned(&self.shared.claims_deferred_until) = None;
        self.wake();
        Ok(replaced)
    }

    pub fn unregister_handler(&self, kind: &str) -> bool {
        write_unpoisoned(&self.shared.handlers)
            .remove(kind)
            .is_some()
    }

    /// Recovers interrupted attempts before any worker can claim work, then starts exactly the
    /// configured number of worker threads. Calling `start` more than once is a no-op.
    pub fn start(&self) -> Result<(), HostError> {
        {
            let mut state = lock_unpoisoned(&self.shared.wake_state);
            if state.shutdown {
                return Err(HostError::new(
                    "TASK_RUNNER_SHUT_DOWN",
                    "task runner has already shut down",
                    false,
                ));
            }
            if state.started {
                return Ok(());
            }
            state.started = true;
            state.generation = state.generation.wrapping_add(1);
        }

        let recovery = match self
            .shared
            .engine
            .recover_interrupted_with_events(STARTUP_RECOVERY_TRACE)
        {
            Ok(recovery) => recovery,
            Err(error) => {
                lock_unpoisoned(&self.shared.wake_state).started = false;
                return Err(error);
            }
        };
        forward_events(&self.shared.event_sink, recovery.emitted_events);

        let mut workers = lock_unpoisoned(&self.workers);
        for worker_id in 0..self.worker_count {
            let shared = Arc::clone(&self.shared);
            let handle = match thread::Builder::new()
                .name(format!("bsaigc-task-{worker_id}"))
                .spawn(move || worker_loop(shared, worker_id))
            {
                Ok(handle) => handle,
                Err(error) => {
                    request_shutdown(&self.shared);
                    let spawned = std::mem::take(&mut *workers);
                    drop(workers);
                    join_workers(spawned)?;
                    return Err(HostError::internal(format!(
                        "spawn task runner worker failed: {error}"
                    )));
                }
            };
            workers.push(handle);
        }
        drop(workers);
        self.shared.wake_signal.notify_all();
        Ok(())
    }

    /// Signals that durable task state may now contain runnable work.
    pub fn wake(&self) {
        signal_work(&self.shared);
    }

    /// Consumes a committed lifecycle event. The ledger remains authoritative; this method only
    /// wakes workers or accelerates cooperative cancellation.
    pub fn notify_event(&self, event: &TaskDomainEvent) {
        match event.event_type {
            TaskEventType::Canceled => {
                self.notify_task_canceled(&event.aggregate_id);
            }
            TaskEventType::Created
            | TaskEventType::Retried
            | TaskEventType::Recovered
            | TaskEventType::Succeeded
            | TaskEventType::Failed => self.wake(),
            TaskEventType::Progressed | TaskEventType::AwaitingApproval => {}
        }
    }

    /// Must be called after the cancel transition commits. It never mutates the ledger itself.
    pub fn notify_task_canceled(&self, task_id: &str) -> bool {
        let tokens = lock_unpoisoned(&self.shared.running)
            .iter()
            .filter(|(key, _)| key.task_id == task_id)
            .map(|(_, token)| token.clone())
            .collect::<Vec<_>>();
        for token in &tokens {
            token.cancel();
        }
        self.wake();
        !tokens.is_empty()
    }

    pub fn active_task_count(&self) -> usize {
        lock_unpoisoned(&self.shared.running).len()
    }

    /// Requests cooperative cancellation, wakes sleepers, and joins all worker threads.
    pub fn shutdown(&self) -> Result<(), HostError> {
        let was_started = lock_unpoisoned(&self.shared.wake_state).started;
        request_shutdown(&self.shared);
        let workers = std::mem::take(&mut *lock_unpoisoned(&self.workers));
        join_workers(workers)?;
        if !was_started {
            return Ok(());
        }
        let recovery = self
            .shared
            .engine
            .recover_interrupted_with_events(SHUTDOWN_RECOVERY_TRACE)?;
        forward_events(&self.shared.event_sink, recovery.emitted_events);
        Ok(())
    }
}

impl Drop for TaskRunner {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("task runner shutdown failed: {error}");
        }
    }
}

fn worker_loop(shared: Arc<RunnerShared>, worker_id: usize) {
    let mut observed_generation = 0;
    loop {
        let Some(generation) = wait_for_work(&shared, observed_generation) else {
            return;
        };
        observed_generation = generation;
        drain_runnable_tasks(&shared, worker_id);
    }
}

fn wait_for_work(shared: &RunnerShared, observed_generation: u64) -> Option<u64> {
    let mut state = lock_unpoisoned(&shared.wake_state);
    while !state.shutdown && state.generation == observed_generation {
        let (next_state, wait_result) = shared
            .wake_signal
            .wait_timeout(state, FALLBACK_WAKE_INTERVAL)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next_state;
        if wait_result.timed_out() {
            break;
        }
    }
    (!state.shutdown).then_some(state.generation)
}

fn drain_runnable_tasks(shared: &Arc<RunnerShared>, worker_id: usize) {
    loop {
        if is_shutting_down(shared) || claims_are_deferred(shared) {
            return;
        }
        let registered_kinds = read_unpoisoned(&shared.handlers)
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        if registered_kinds.is_empty() {
            return;
        }
        let claim_trace = runner_trace(worker_id, "claim");
        let claim = match shared
            .engine
            .claim_next_runnable_for_kinds_with_events(&registered_kinds, &claim_trace)
        {
            Ok(claim) => claim,
            Err(error) => {
                eprintln!("task runner claim failed: {error}");
                return;
            }
        };
        forward_events(&shared.event_sink, claim.emitted_events);
        let Some(task) = claim.task else {
            return;
        };
        execute_claimed_task(shared, worker_id, task);
    }
}

fn execute_claimed_task(shared: &Arc<RunnerShared>, worker_id: usize, task: TaskRecord) {
    let key = AttemptKey {
        task_id: task.id.clone(),
        attempt: task.attempt,
    };
    let cancellation = CancellationToken::new();
    lock_unpoisoned(&shared.running).insert(key.clone(), cancellation.clone());

    let handler = read_unpoisoned(&shared.handlers).get(&task.kind).cloned();
    let handler_missing = handler.is_none();
    let trace_id = runner_trace(worker_id, "execute");
    let result = if let Some(handler) = handler {
        let context = HandlerContext {
            task: task.id.clone(),
            attempt: task.attempt,
            input: task.input.clone(),
            project: task.project_id.clone(),
            cancel: cancellation.clone(),
            progress: ProgressReporter {
                engine: Arc::clone(&shared.engine),
                task: task.id.clone(),
                attempt: task.attempt,
                cancel: cancellation,
                event_sink: Arc::clone(&shared.event_sink),
                trace_id: trace_id.clone(),
            },
        };
        match panic::catch_unwind(AssertUnwindSafe(|| handler.execute(context))) {
            Ok(result) => result,
            Err(payload) => Err(TaskHandlerError::structured(
                "TASK_HANDLER_PANICKED",
                format!("task handler panicked: {}", panic_message(payload.as_ref())),
                false,
            )),
        }
    } else {
        defer_claims(shared);
        Err(TaskHandlerError::structured(
            "TASK_HANDLER_NOT_REGISTERED",
            format!("no task handler registered for kind {}", task.kind),
            false,
        ))
    };

    if is_shutting_down(shared) || !attempt_is_active(&shared.engine, &task) {
        lock_unpoisoned(&shared.running).remove(&key);
        return;
    }

    let outcome = match result {
        Ok(output) => {
            shared
                .engine
                .finish_success_with_events(&task.id, task.attempt, output, &trace_id)
        }
        Err(error) => shared.engine.finish_handler_failure_with_events(
            &task.id,
            task.attempt,
            error.encoded(),
            error.retryable(),
            &trace_id,
        ),
    };
    match outcome {
        Ok(outcome) => forward_lifecycle_outcome(shared, outcome),
        Err(error) if error.code == "TASK_ATTEMPT_STALE" => {}
        Err(error) => eprintln!(
            "task runner finalize failed: task_id={} attempt={} error={error}",
            task.id, task.attempt
        ),
    }
    lock_unpoisoned(&shared.running).remove(&key);
    if !handler_missing {
        signal_work(shared);
    }
}

fn attempt_is_active(engine: &TaskEngine, claimed: &TaskRecord) -> bool {
    engine.get_task(&claimed.id).is_ok_and(|current| {
        matches!(current.status, TaskStatus::Running) && current.attempt == claimed.attempt
    })
}

fn forward_lifecycle_outcome(shared: &RunnerShared, outcome: TaskLifecycleOutcome) {
    forward_events(&shared.event_sink, outcome.emitted_events);
}

fn forward_events(
    sink: &RwLock<Option<TaskLifecycleEventSink>>,
    events: impl IntoIterator<Item = TaskDomainEvent>,
) {
    let sink = read_unpoisoned(sink).clone();
    let Some(sink) = sink else {
        return;
    };
    for event in events {
        match panic::catch_unwind(AssertUnwindSafe(|| sink(event))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => eprintln!("task lifecycle event sink failed: {error}"),
            Err(_) => eprintln!("task lifecycle event sink panicked"),
        }
    }
}

fn signal_work(shared: &RunnerShared) {
    let mut state = lock_unpoisoned(&shared.wake_state);
    if state.shutdown {
        return;
    }
    state.generation = state.generation.wrapping_add(1);
    drop(state);
    shared.wake_signal.notify_all();
}

fn request_shutdown(shared: &RunnerShared) {
    {
        let mut state = lock_unpoisoned(&shared.wake_state);
        state.shutdown = true;
        state.generation = state.generation.wrapping_add(1);
    }
    for token in lock_unpoisoned(&shared.running).values() {
        token.cancel();
    }
    shared.wake_signal.notify_all();
}

fn join_workers(workers: Vec<JoinHandle<()>>) -> Result<(), HostError> {
    let mut panicked = Vec::new();
    for worker in workers {
        let name = worker.thread().name().unwrap_or("unnamed").to_string();
        if worker.join().is_err() {
            panicked.push(name);
        }
    }
    if panicked.is_empty() {
        Ok(())
    } else {
        Err(HostError::internal(format!(
            "task runner workers panicked: {}",
            panicked.join(", ")
        )))
    }
}

fn is_shutting_down(shared: &RunnerShared) -> bool {
    lock_unpoisoned(&shared.wake_state).shutdown
}

fn defer_claims(shared: &RunnerShared) {
    *lock_unpoisoned(&shared.claims_deferred_until) =
        Some(Instant::now() + UNREGISTERED_HANDLER_BACKOFF);
}

fn claims_are_deferred(shared: &RunnerShared) -> bool {
    let mut deferred_until = lock_unpoisoned(&shared.claims_deferred_until);
    match *deferred_until {
        Some(until) if until > Instant::now() => true,
        Some(_) => {
            *deferred_until = None;
            false
        }
        None => false,
    }
}

fn runner_trace(worker_id: usize, stage: &str) -> String {
    format!("task-runner:{worker_id}:{stage}:{}", Uuid::new_v4())
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn read_unpoisoned<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_unpoisoned<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CreateTaskPayload, OperationContext, TaskCommandEnvelope, TaskEventType, TaskPriority,
        TaskReplayPolicy, PROTOCOL_VERSION,
    };
    use rusqlite::Connection;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    fn engine() -> Arc<TaskEngine> {
        Arc::new(TaskEngine::from_connection(Connection::open_in_memory().unwrap()).unwrap())
    }

    fn create_task(
        engine: &TaskEngine,
        kind: &str,
        policy: TaskReplayPolicy,
        max_attempts: u32,
        dependencies: Vec<String>,
    ) -> TaskRecord {
        engine
            .execute_command(TaskCommandEnvelope::Create {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: OperationContext {
                    actor_id: "task-runner-test".to_string(),
                    account_id: None,
                    project_id: None,
                    window_id: "test-window".to_string(),
                    trace_id: Uuid::new_v4().to_string(),
                },
                payload: CreateTaskPayload {
                    kind: kind.to_string(),
                    project_id: None,
                    input: json!({}),
                    priority: TaskPriority::Normal,
                    replay_policy: policy,
                    max_attempts,
                    dependency_task_ids: dependencies,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: None,
                deadline_at: None,
            })
            .unwrap()
            .response
            .task
    }

    fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
        let started = Instant::now();
        while !predicate() {
            assert!(started.elapsed() < timeout, "condition timed out");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn fixed_slots_bound_parallel_handler_execution() {
        let engine = engine();
        let runner = TaskRunner::new(Arc::clone(&engine), 2).unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        runner
            .register_handler("parallel", {
                let active = Arc::clone(&active);
                let maximum = Arc::clone(&maximum);
                let started = Arc::clone(&started);
                let gate = Arc::clone(&gate);
                move |_| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(now, Ordering::SeqCst);
                    started.fetch_add(1, Ordering::SeqCst);
                    let (lock, signal) = &*gate;
                    let mut released = lock_unpoisoned(lock);
                    while !*released {
                        released = signal.wait(released).unwrap();
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(json!({ "ok": true }))
                }
            })
            .unwrap();
        runner.start().unwrap();
        let tasks = (0..4)
            .map(|_| create_task(&engine, "parallel", TaskReplayPolicy::Never, 1, vec![]))
            .collect::<Vec<_>>();
        runner.wake();

        wait_until(Duration::from_secs(2), || {
            started.load(Ordering::SeqCst) == 2
        });
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        {
            let (lock, signal) = &*gate;
            *lock_unpoisoned(lock) = true;
            signal.notify_all();
        }
        wait_until(Duration::from_secs(3), || {
            tasks.iter().all(|task| {
                matches!(
                    engine.get_task(&task.id).unwrap().status,
                    TaskStatus::Succeeded
                )
            })
        });
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        runner.shutdown().unwrap();
    }

    #[test]
    fn dag_child_runs_only_after_parent_succeeds() {
        let engine = engine();
        let runner = TaskRunner::new(Arc::clone(&engine), 2).unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        runner
            .register_handler("parent", {
                let order = Arc::clone(&order);
                move |_| {
                    lock_unpoisoned(&order).push("parent");
                    Ok(json!({ "parent": true }))
                }
            })
            .unwrap();
        runner
            .register_handler("child", {
                let order = Arc::clone(&order);
                move |_| {
                    lock_unpoisoned(&order).push("child");
                    Ok(json!({ "child": true }))
                }
            })
            .unwrap();
        runner.start().unwrap();
        let parent = create_task(&engine, "parent", TaskReplayPolicy::Never, 1, vec![]);
        let child = create_task(
            &engine,
            "child",
            TaskReplayPolicy::Never,
            1,
            vec![parent.id.clone()],
        );
        runner.wake();

        wait_until(Duration::from_secs(2), || {
            matches!(
                engine.get_task(&child.id).unwrap().status,
                TaskStatus::Succeeded
            )
        });
        assert_eq!(*lock_unpoisoned(&order), vec!["parent", "child"]);
        runner.shutdown().unwrap();
    }

    #[test]
    fn committed_cancel_reaches_cooperative_handler() {
        let engine = engine();
        let runner = TaskRunner::new(Arc::clone(&engine), 2).unwrap();
        let handler_stopped = Arc::new(AtomicBool::new(false));
        runner
            .register_handler("cancel", {
                let handler_stopped = Arc::clone(&handler_stopped);
                move |context: HandlerContext| {
                    context.cancel.wait_cancelled(Duration::from_secs(2));
                    handler_stopped.store(true, Ordering::Release);
                    Err(TaskHandlerError::new("canceled"))
                }
            })
            .unwrap();
        runner.start().unwrap();
        let task = create_task(&engine, "cancel", TaskReplayPolicy::Never, 1, vec![]);
        runner.wake();
        wait_until(Duration::from_secs(2), || {
            matches!(
                engine.get_task(&task.id).unwrap().status,
                TaskStatus::Running
            )
        });
        let running = engine.get_task(&task.id).unwrap();
        engine.cancel_task(&task.id, running.revision).unwrap();
        assert!(runner.notify_task_canceled(&task.id));

        wait_until(Duration::from_secs(2), || {
            handler_stopped.load(Ordering::Acquire)
        });
        assert!(matches!(
            engine.get_task(&task.id).unwrap().status,
            TaskStatus::Canceled
        ));
        runner.shutdown().unwrap();
    }

    #[test]
    fn handler_panic_isolated_and_worker_keeps_running() {
        let engine = engine();
        let runner = TaskRunner::new(Arc::clone(&engine), 2).unwrap();
        runner
            .register_handler("panic", |_| -> TaskHandlerResult { panic!("boom") })
            .unwrap();
        runner
            .register_handler("healthy", |_| Ok(json!({ "survived": true })))
            .unwrap();
        runner.start().unwrap();
        let panicked = create_task(&engine, "panic", TaskReplayPolicy::Never, 1, vec![]);
        let healthy = create_task(&engine, "healthy", TaskReplayPolicy::Never, 1, vec![]);
        runner.wake();

        wait_until(Duration::from_secs(2), || {
            matches!(
                engine.get_task(&healthy.id).unwrap().status,
                TaskStatus::Succeeded
            )
        });
        let panicked = engine.get_task(&panicked.id).unwrap();
        assert!(matches!(panicked.status, TaskStatus::Failed));
        assert!(panicked.last_error.unwrap().contains("panicked: boom"));
        runner.shutdown().unwrap();
    }

    #[test]
    fn lifecycle_outcomes_are_forwarded() {
        let engine = engine();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink: TaskLifecycleEventSink = {
            let events = Arc::clone(&events);
            Arc::new(move |event| {
                lock_unpoisoned(&events).push(event);
                Ok(())
            })
        };
        let runner = TaskRunner::with_event_sink(Arc::clone(&engine), 2, sink).unwrap();
        runner
            .register_handler("events", |context: HandlerContext| {
                context.progress.report(40)?;
                Ok(json!({ "done": true }))
            })
            .unwrap();
        runner.start().unwrap();
        let task = create_task(&engine, "events", TaskReplayPolicy::Never, 1, vec![]);
        runner.wake();

        wait_until(Duration::from_secs(2), || {
            matches!(
                engine.get_task(&task.id).unwrap().status,
                TaskStatus::Succeeded
            )
        });
        let event_types = lock_unpoisoned(&events)
            .iter()
            .map(|event| event.event_type.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                TaskEventType::Progressed,
                TaskEventType::Progressed,
                TaskEventType::Succeeded
            ]
        );
        runner.shutdown().unwrap();
    }

    #[test]
    fn shutdown_cancels_handlers_and_joins_idempotently() {
        let engine = engine();
        let runner = TaskRunner::new(Arc::clone(&engine), 2).unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        runner
            .register_handler("shutdown", {
                let stopped = Arc::clone(&stopped);
                move |context: HandlerContext| {
                    context.cancel.wait_cancelled(Duration::from_secs(2));
                    stopped.store(true, Ordering::Release);
                    Err(TaskHandlerError::new("host shutdown"))
                }
            })
            .unwrap();
        runner.start().unwrap();
        let task = create_task(&engine, "shutdown", TaskReplayPolicy::Safe, 2, vec![]);
        runner.wake();
        wait_until(Duration::from_secs(2), || runner.active_task_count() == 1);

        runner.shutdown().unwrap();
        assert!(stopped.load(Ordering::Acquire));
        assert!(matches!(
            engine.get_task(&task.id).unwrap().status,
            TaskStatus::Queued
        ));
        runner.shutdown().unwrap();
    }
}
