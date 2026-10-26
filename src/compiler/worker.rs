//! Background compiler thread that debounces compilation and sends results via channels.

#![allow(dead_code)]

use crate::compiler::engine::{CompileCancellationToken, CompileOptions, CompileResult, CompilerEngine, EngineKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub struct CompileTask {
    pub id: u64,
    pub source: String,
    pub engine: EngineKind,
    pub work_dir: Option<PathBuf>,
    pub doc_name: Option<String>,
    pub allow_shell_escape: bool,
    pub shadow_subdocument: Option<(PathBuf, String)>,
    pub timestamp: Instant,
}

pub struct CompileOutput {
    pub id: u64,
    pub result: std::result::Result<CompileResult, String>,
}

/// Internal command sent to the compilation worker thread.
enum WorkerCommand {
    Submit(CompileTask),
    Cancel,
}

/// Asynchronous compilation worker manager.
pub struct CompilerWorker {
    cmd_sender: Sender<WorkerCommand>,
    output_receiver: Receiver<CompileOutput>,
    cancel_token: CompileCancellationToken,
    next_id: Arc<AtomicU64>,
    is_busy: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for CompilerWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerWorker {
    /// Spawns the background worker thread with a 350ms debounce window.
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = channel::<WorkerCommand>();
        let (output_tx, output_rx) = channel::<CompileOutput>();
        let next_id = Arc::new(AtomicU64::new(1));
        let is_busy = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_token = CompileCancellationToken::new();

        let is_busy_worker = is_busy.clone();
        let cancel_token_worker = cancel_token.clone();

        thread::spawn(move || {
            let mut pending_task: Option<CompileTask> = None;
            let debounce_duration = Duration::from_millis(350);

            loop {
                if let Some(task) = &pending_task {
                    let elapsed = task.timestamp.elapsed();
                    if elapsed >= debounce_duration {
                        let current_task = pending_task.take().unwrap();
                        cancel_token_worker.reset();
                        is_busy_worker.store(true, Ordering::SeqCst);

                        let options = CompileOptions {
                            work_dir: current_task.work_dir,
                            doc_name: current_task.doc_name,
                            allow_shell_escape: current_task.allow_shell_escape,
                            shadow_subdocument: current_task.shadow_subdocument,
                            cancel_token: Some(cancel_token_worker.clone()),
                        };
                        let result = CompilerEngine::compile_with_options(
                            &current_task.source,
                            current_task.engine,
                            &options,
                        )
                        .map_err(|e| e.to_string());

                        is_busy_worker.store(false, Ordering::SeqCst);

                        let _ = output_tx.send(CompileOutput {
                            id: current_task.id,
                            result,
                        });
                        continue;
                    }

                    let wait_time = debounce_duration.saturating_sub(elapsed);
                    match cmd_rx.recv_timeout(wait_time) {
                        Ok(WorkerCommand::Submit(newer_task)) => {
                            pending_task = Some(newer_task);
                        }
                        Ok(WorkerCommand::Cancel) => {
                            pending_task = None;
                            is_busy_worker.store(false, Ordering::SeqCst);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match cmd_rx.recv() {
                        Ok(WorkerCommand::Submit(task)) => {
                            pending_task = Some(task);
                        }
                        Ok(WorkerCommand::Cancel) => {
                            pending_task = None;
                            is_busy_worker.store(false, Ordering::SeqCst);
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        Self {
            cmd_sender: cmd_tx,
            output_receiver: output_rx,
            cancel_token,
            next_id,
            is_busy,
        }
    }

    /// Submits LaTeX code for compilation (debounced). Returns the assigned task ID.
    pub fn submit(
        &self,
        source: String,
        engine: EngineKind,
        work_dir: Option<PathBuf>,
    ) -> u64 {
        self.submit_named(source, engine, work_dir, None)
    }

    /// Submits LaTeX code with document filename for compilation (debounced). Returns the assigned task ID.
    pub fn submit_named(
        &self,
        source: String,
        engine: EngineKind,
        work_dir: Option<PathBuf>,
        doc_name: Option<String>,
    ) -> u64 {
        self.submit_with_options(
            source,
            engine,
            CompileOptions {
                work_dir,
                doc_name,
                allow_shell_escape: false,
                shadow_subdocument: None,
                cancel_token: None,
            },
        )
    }

    /// Submits LaTeX code with full [`CompileOptions`] (debounced). Returns the assigned task ID.
    pub fn submit_with_options(
        &self,
        source: String,
        engine: EngineKind,
        options: CompileOptions,
    ) -> u64 {
        self.cancel_token.reset();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let task = CompileTask {
            id,
            source,
            engine,
            work_dir: options.work_dir,
            doc_name: options.doc_name,
            allow_shell_escape: options.allow_shell_escape,
            shadow_subdocument: options.shadow_subdocument,
            timestamp: Instant::now(),
        };
        let _ = self.cmd_sender.send(WorkerCommand::Submit(task));
        id
    }

    /// Non-blocking check for completed compilation results.
    pub fn try_receive(&self) -> Option<CompileOutput> {
        match self.output_receiver.try_recv() {
            Ok(output) => Some(output),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Returns true if the background worker is currently executing a compilation command.
    pub fn is_compiling(&self) -> bool {
        self.is_busy.load(Ordering::SeqCst)
    }

    /// Cancels any active compilation, terminates running compiler processes, and clears debounced tasks.
    pub fn cancel(&self) {
        // Signal cancellation to abort active process immediately
        self.cancel_token.cancel();
        // Clear any pending debounce task in worker thread
        let _ = self.cmd_sender.send(WorkerCommand::Cancel);
        // Clear busy status
        self.is_busy.store(false, Ordering::SeqCst);
        // Drain any already queued output messages
        while self.output_receiver.try_recv().is_ok() {}
    }

    /// Allocates and returns the next task ID, useful to invalidate superseded or cancelled tasks.
    pub fn next_task_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_worker_debounce_cancel_and_recovery() {
        let worker = CompilerWorker::new();
        assert!(!worker.is_compiling());

        // Submit a task
        let id1 = worker.submit(
            "\\documentclass{article}\\begin{document}Hello\\end{document}".to_string(),
            EngineKind::Native,
            None,
        );
        assert_eq!(id1, 1);

        // Cancel immediately while in debounce window
        worker.cancel();
        assert!(!worker.is_compiling());

        // Wait past debounce duration to ensure the cancelled task was aborted
        std::thread::sleep(Duration::from_millis(450));
        assert!(worker.try_receive().is_none());
        assert!(!worker.is_compiling());

        // Submit a new task after cancellation and verify worker executes and delivers result
        let id2 = worker.submit(
            "\\documentclass{article}\\begin{document}Recovered\\end{document}".to_string(),
            EngineKind::Native,
            None,
        );
        assert_eq!(id2, 2);

        // Poll for completion (within 4s to account for parallel test runner load)
        let start = Instant::now();
        let mut got_output = None;
        while start.elapsed() < Duration::from_millis(4000) {
            if let Some(out) = worker.try_receive() {
                got_output = Some(out);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let output = got_output.expect("Should receive output for second task");
        assert_eq!(output.id, id2);
        assert!(output.result.is_ok());
        assert!(!worker.is_compiling());
    }
}
