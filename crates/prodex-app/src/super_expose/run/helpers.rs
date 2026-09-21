use super::*;

pub(super) fn prune_terminal_runs(runs: &mut BTreeMap<String, RunRecord>) {
    while runs.values().filter(|run| run.state.terminal()).count() > MAX_RETAINED_TERMINAL_RUNS {
        let Some(oldest_id) = runs
            .iter()
            .filter(|(_, run)| run.state.terminal())
            .min_by_key(|(_, run)| (run.finished_at.unwrap_or(run.created_at), run.created_at))
            .map(|(run_id, _)| run_id.clone())
        else {
            break;
        };
        runs.remove(&oldest_id);
    }
}

pub(super) struct SpawnedChildPipes {
    pub(super) stdout: Option<ChildStdout>,
    pub(super) stderr: Option<ChildStderr>,
    pub(super) stdin: Option<ChildStdin>,
}

pub(super) fn spawn_registered_child(
    mut command: Command,
    child_slot: &Arc<Mutex<Option<Child>>>,
) -> std::result::Result<SpawnedChildPipes, String> {
    let mut child = command.spawn().map_err(|error| format!("spawn: {error}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    let Ok(mut slot) = child_slot.lock() else {
        let _ = terminate_child_process_tree(&mut child, true);
        return Err("child state unavailable".to_string());
    };
    *slot = Some(child);
    Ok(SpawnedChildPipes {
        stdout,
        stderr,
        stdin,
    })
}

pub(super) fn spawn_child_readers(
    manager: RunManager,
    run_id: &str,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
) -> Vec<thread::JoinHandle<()>> {
    [
        stdout.map(|reader| spawn_reader(manager.clone(), run_id.to_string(), reader, "stdout")),
        stderr.map(|reader| spawn_reader(manager, run_id.to_string(), reader, "stderr")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(super) fn write_child_task(mut stdin: Option<ChildStdin>, task: &str, cancel: &AtomicBool) {
    let Some(mut stdin) = stdin.take() else {
        return;
    };
    if stdin.write_all(task.as_bytes()).is_err() || stdin.flush().is_err() {
        cancel.store(true, Ordering::SeqCst);
    }
}

pub(super) fn poll_child_status(
    child_slot: &Arc<Mutex<Option<Child>>>,
    cancel: &AtomicBool,
) -> Option<ExitStatus> {
    loop {
        if cancel.load(Ordering::SeqCst) {
            terminate_registered_child(child_slot);
        }
        let polled = child_slot
            .lock()
            .ok()
            .and_then(|mut slot| slot.as_mut().map(Child::try_wait));
        match polled {
            Some(Ok(Some(status))) => return Some(status),
            Some(Ok(None)) => thread::sleep(Duration::from_millis(25)),
            Some(Err(_)) | None => return None,
        }
    }
}

pub(super) fn terminate_registered_child(child_slot: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut slot) = child_slot.lock()
        && let Some(child) = slot.as_mut()
    {
        let _ = terminate_child_process_tree(child, true);
    }
}

pub(super) fn clear_child_slot(child_slot: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut slot) = child_slot.lock() {
        slot.take();
    }
}

pub(super) fn join_child_readers(readers: Vec<thread::JoinHandle<()>>) {
    for reader in readers {
        let _ = reader.join();
    }
}

pub(super) fn apply_run_terminal_state(
    record: &mut RunRecord,
    cancelled: bool,
    status: Option<ExitStatus>,
) {
    if cancelled {
        record.state = RunState::Cancelled;
        return;
    }
    let Some(status) = status else {
        record.state = RunState::StartFailed;
        return;
    };
    record.exit_status = status.code();
    record.state = if status.success() {
        RunState::Succeeded
    } else {
        RunState::Failed
    };
}

pub(super) fn spawn_reader(
    manager: RunManager,
    run_id: String,
    mut reader: impl Read + Send + 'static,
    event_type: &'static str,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => manager.append(&run_id, event_type, &buffer[..size]),
                Err(_) => break,
            }
        }
    })
}

pub(super) fn append_output(record: &mut RunRecord, event_type: &str, bytes: &[u8]) {
    let text = bounded_redacted_text(bytes, MAX_RUN_EVENT_TEXT_BYTES);
    if text.is_empty() {
        return;
    }
    if record.output.len() < OUTPUT_MAX_BYTES {
        let remaining = OUTPUT_MAX_BYTES - record.output.len();
        let mut end = text.len().min(remaining);
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        record.output.push_str(&text[..end]);
        if end < text.len() {
            record.output_truncated = true;
        }
    } else {
        record.output_truncated = true;
    }
    push_event(record, event_type, &text);
    if record.output_truncated
        && !record
            .events
            .iter()
            .any(|event| event.event_type == "output_truncated")
    {
        push_event(record, "output_truncated", "output limit reached");
    }
}

pub(super) fn events_page(record: &RunRecord, after_seq: u64, limit: usize) -> RunEvents {
    let first_seq = record
        .events
        .front()
        .map_or(record.next_seq, |event| event.seq);
    RunEvents {
        events: record
            .events
            .iter()
            .filter(|event| event.seq > after_seq)
            .take(limit.min(MAX_RUN_EVENTS))
            .cloned()
            .collect(),
        next_seq: record.next_seq,
        truncated: after_seq.saturating_add(1) < first_seq,
    }
}

pub(super) fn push_event(record: &mut RunRecord, event_type: &str, text: &str) {
    let text = bounded_event_text(text);
    record.events.push_back(RunEvent {
        seq: record.next_seq,
        event_type: event_type.to_string(),
        text,
    });
    record.next_seq = record.next_seq.saturating_add(1);
    while record.events.len() > MAX_RUN_EVENTS {
        record.events.pop_front();
    }
}

pub(super) fn bounded_event_text(text: &str) -> String {
    if text.len() <= MAX_RUN_EVENT_TEXT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_RUN_EVENT_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text[..end].to_string()
}

pub(super) fn summary_json(run_id: &str, record: &RunRecord) -> Value {
    json!({
        "run_id": run_id,
        "state": record.state.as_str(),
        "created_at": record.created_at,
        "started_at": record.started_at,
        "finished_at": record.finished_at,
        "exit_status": record.exit_status,
        "provider": record.provider,
        "model": record.model,
        "reasoning_effort": record.reasoning_effort,
        "cancellation_requested": record.cancel.load(Ordering::SeqCst),
    })
}

pub(super) fn new_run_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).context("failed to generate expose run id")?;
    Ok(format!(
        "spr_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}
