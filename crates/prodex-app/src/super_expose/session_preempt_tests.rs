use super::session_prompt_write::{
    ExistingSessionPromptWrite, QueueControl, QueueInvocation, QueuePreemptResult, ResolvedTarget,
    SessionPreemptRequest, SessionPromptWriteError, SessionPromptWriteRequest,
    SessionPromptWriteService,
};
use super::session_prompt_write_tests::{fixture, process_inspector};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

const TURN_A: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216e4";

struct ServiceQueueControl {
    rollout: PathBuf,
    state: Arc<Mutex<ServiceQueueState>>,
    queue_entered: Option<Arc<Barrier>>,
    queue_release: Option<Arc<Barrier>>,
    block_queue: AtomicBool,
}
struct ServiceQueueState {
    active_turn: Option<String>,
    queue: Vec<(String, String)>,
    next_id: usize,
    enqueue_after_boundary: Option<String>,
}
impl QueueControl for ServiceQueueControl {
    fn check_capability(&self, _target: &ResolvedTarget) -> Result<(), SessionPromptWriteError> {
        Ok(())
    }

    fn persisted_thread(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<bool, SessionPromptWriteError> {
        Ok(true)
    }

    fn rollout_path(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<Option<PathBuf>, SessionPromptWriteError> {
        Ok(Some(self.rollout.clone()))
    }

    fn queue_once(&self, _target: &ResolvedTarget, message: &str) -> QueueInvocation {
        let id = {
            let mut state = self.state.lock().unwrap();
            state.next_id += 1;
            let id = format!("submission-{}", state.next_id);
            state.queue.push((id.clone(), message.to_string()));
            id
        };
        if self.block_queue.swap(false, Ordering::SeqCst) {
            if let Some(barrier) = &self.queue_entered {
                barrier.wait();
            }
            if let Some(barrier) = &self.queue_release {
                barrier.wait();
            }
        }
        QueueInvocation::accepted(Some(0), Some(id.clone()), Some(id), true)
    }

    fn preempt(
        &self,
        _target: &ResolvedTarget,
    ) -> Result<QueuePreemptResult, SessionPromptWriteError> {
        let mut state = self.state.lock().unwrap();
        let current_turn_id = state.active_turn.take();
        let cancelled_submission_ids = state.queue.drain(..).map(|(id, _)| id).collect::<Vec<_>>();
        let queue_empty_at_boundary = true;
        if let Some(message) = state.enqueue_after_boundary.take() {
            state.next_id += 1;
            let id = format!("submission-{}", state.next_id);
            state.queue.push((id, message));
        }
        Ok(QueuePreemptResult {
            current_turn_id: current_turn_id.clone(),
            current_turn_interrupted: current_turn_id.is_some(),
            cancelled_submission_ids,
            remaining_submission_ids: state.queue.iter().map(|(id, _)| id.clone()).collect(),
            queue_empty_at_boundary,
            session_ready: state.active_turn.is_none() && state.queue.is_empty(),
        })
    }
}
fn preempt_request(fixture: &super::session_prompt_write_tests::Fixture) -> SessionPreemptRequest {
    SessionPreemptRequest {
        workspace_root: fixture.workspace.clone(),
        cwd: None,
        prodex_pid: None,
        thread_id: None,
        binding_key: "preempt-test".to_string(),
    }
}

fn service_queue(
    fixture: &super::session_prompt_write_tests::Fixture,
    active_turn: Option<&str>,
    queue: &[(&str, &str)],
) -> (ServiceQueueControl, Arc<Mutex<ServiceQueueState>>) {
    let state = Arc::new(Mutex::new(ServiceQueueState {
        active_turn: active_turn.map(str::to_string),
        queue: queue
            .iter()
            .map(|(id, message)| ((*id).to_string(), (*message).to_string()))
            .collect(),
        next_id: queue.len(),
        enqueue_after_boundary: None,
    }));
    (
        ServiceQueueControl {
            rollout: fixture.rollout.clone(),
            state: Arc::clone(&state),
            queue_entered: None,
            queue_release: None,
            block_queue: AtomicBool::new(false),
        },
        state,
    )
}

#[test]
fn preempt_interrupts_current_turn_and_cancels_all_pending_prompts() {
    let fixture = fixture();
    let (queue, state) = service_queue(
        &fixture,
        Some(TURN_A),
        &[("queued-a", "first"), ("queued-b", "second")],
    );
    let service = SessionPromptWriteService::with_adapters(process_inspector(&fixture), queue);

    let result = service.preempt(preempt_request(&fixture)).unwrap();

    assert_eq!(result.current_turn_id.as_deref(), Some(TURN_A));
    assert!(result.current_turn_interrupted);
    assert_eq!(result.cancelled_submission_ids, ["queued-a", "queued-b"]);
    assert!(result.remaining_submission_ids.is_empty());
    assert!(result.queue_empty_at_boundary);
    assert!(result.session_ready);
    assert!(state.lock().unwrap().queue.is_empty());
}

#[test]
fn preempt_without_current_turn_cancels_queue_and_repeats_safely() {
    let fixture = fixture();
    let (queue, state) = service_queue(
        &fixture,
        None,
        &[("queued-a", "first"), ("queued-b", "second")],
    );
    let service = SessionPromptWriteService::with_adapters(process_inspector(&fixture), queue);

    let first = service.preempt(preempt_request(&fixture)).unwrap();
    let second = service.preempt(preempt_request(&fixture)).unwrap();

    assert!(first.current_turn_id.is_none());
    assert!(!first.current_turn_interrupted);
    assert_eq!(first.cancelled_submission_ids.len(), 2);
    assert!(first.session_ready);
    assert!(second.current_turn_id.is_none());
    assert!(!second.current_turn_interrupted);
    assert!(second.cancelled_submission_ids.is_empty());
    assert!(second.session_ready);
    assert_eq!(second.generation, 2);
    assert!(state.lock().unwrap().queue.is_empty());
}

#[test]
fn enqueue_before_preempt_is_cancelled_and_enqueue_after_boundary_survives() {
    let fixture = fixture();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let state = Arc::new(Mutex::new(ServiceQueueState {
        active_turn: Some(TURN_A.to_string()),
        queue: Vec::new(),
        next_id: 0,
        enqueue_after_boundary: None,
    }));
    let queue = ServiceQueueControl {
        rollout: fixture.rollout.clone(),
        state: Arc::clone(&state),
        queue_entered: Some(Arc::clone(&entered)),
        queue_release: Some(Arc::clone(&release)),
        block_queue: AtomicBool::new(true),
    };
    let service = Arc::new(SessionPromptWriteService::with_adapters(
        process_inspector(&fixture),
        queue,
    ));
    let write_service = Arc::clone(&service);
    let write_fixture = fixture.workspace.clone();
    let writer = thread::spawn(move || {
        write_service.write(SessionPromptWriteRequest {
            workspace_root: write_fixture,
            message: "queued-before-preempt".to_string(),
            cwd: None,
            prodex_pid: None,
            thread_id: None,
            binding_key: "write-before".to_string(),
        })
    });
    entered.wait();
    let preempt_service = Arc::clone(&service);
    let preempt_fixture = fixture.workspace.clone();
    let preemptor = thread::spawn(move || {
        preempt_service.preempt(SessionPreemptRequest {
            workspace_root: preempt_fixture,
            cwd: None,
            prodex_pid: None,
            thread_id: None,
            binding_key: "preempt-after-write".to_string(),
        })
    });
    release.wait();

    let written = writer.join().unwrap().unwrap();
    let preempted = preemptor.join().unwrap().unwrap();
    assert_eq!(written.submission_id.as_deref(), Some("submission-1"));
    assert_eq!(preempted.cancelled_submission_ids, ["submission-1"]);
    assert!(preempted.current_turn_interrupted);
    assert!(state.lock().unwrap().queue.is_empty());

    let after = service
        .write(SessionPromptWriteRequest {
            workspace_root: fixture.workspace.clone(),
            message: "queued-after-boundary".to_string(),
            cwd: None,
            prodex_pid: None,
            thread_id: None,
            binding_key: "write-after".to_string(),
        })
        .unwrap();
    assert_eq!(after.submission_id.as_deref(), Some("submission-2"));
    assert_eq!(
        state.lock().unwrap().queue,
        [(
            "submission-2".to_string(),
            "queued-after-boundary".to_string()
        )]
    );
}
