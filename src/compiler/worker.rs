//! # Asynchronous Compilation Worker Thread
//!
//! Provides [`CompilerWorker`], a background worker thread that receives compilation tasks,
//! debounces rapid keystroke events (350ms window), executes compilation out-of-process or
//! via the native engine, and delivers results through channels without blocking the UI.

#![allow(dead_code)]

use crate::compiler::engine::{CompileResult, CompilerEngine, EngineKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// A compilation job queued for processing.
pub struct CompileTask {
    /// Unique monotonically increasing task identifier
    pub id: u64,
    /// LaTeX document source code
    pub source: String,
    /// Compiler engine backend to invoke
    pub engine: EngineKind,
    /// Optional working directory for resolving images, inputs, and relative assets
    pub work_dir: Option<PathBuf>,
    /// Optional document file name used to name the output PDF
    pub doc_name: Option<String>,
    /// Creation instant used for debouncing
    pub timestamp: Instant,
}

/// Compilation result message returned by the worker thread.
pub struct CompileOutput {
    /// Identifier matching the original [`CompileTask`]
    pub id: u64,
    /// Outcome of the compilation
    pub result: std::result::Result<CompileResult, String>,
}

/// Asynchronous compilation worker manager.
pub struct CompilerWorker {
    task_sender: Sender<CompileTask>,
    output_receiver: Receiver<CompileOutput>,
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
        let (task_tx, task_rx) = channel::<CompileTask>();
        let (output_tx, output_rx) = channel::<CompileOutput>();
        let next_id = Arc::new(AtomicU64::new(1));
        let is_busy = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let is_busy_worker = is_busy.clone();

        thread::spawn(move || {
            let mut pending_task: Option<CompileTask> = None;
            let debounce_duration = Duration::from_millis(350);

            loop {
                if let Some(task) = &pending_task {
                    let elapsed = task.timestamp.elapsed();
                    if elapsed >= debounce_duration {
                        let current_task = pending_task.take().unwrap();
                        is_busy_worker.store(true, Ordering::SeqCst);

                        let result = CompilerEngine::compile_named(
                            &current_task.source,
                            current_task.engine,
                            current_task.work_dir.as_deref(),
                            current_task.doc_name.as_deref(),
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
                    match task_rx.recv_timeout(wait_time) {
                        Ok(newer_task) => {
                            pending_task = Some(newer_task);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match task_rx.recv() {
                        Ok(task) => {
                            pending_task = Some(task);
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        Self {
            task_sender: task_tx,
            output_receiver: output_rx,
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
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let task = CompileTask {
            id,
            source,
            engine,
            work_dir,
            doc_name,
            timestamp: Instant::now(),
        };
        let _ = self.task_sender.send(task);
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
}
