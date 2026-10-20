//! 交给一次调用的任务端口（`miyu_tool::JobPort`）：领编号、建输出文件、登记进表，起三个线程，一个读输出写文件，一个等它
//! 结束、存 blob、报给 actor，一个看它跑没跑过最长时间。进程活多久不定，所以用自己的线程，不占 tokio 的阻塞线程池。
//!
//! 上限（施工 7-12 下，[`JobLimits`]）：跑过最长时间的、输出写过上限的，照停掉的规矩整组杀、存输出，报 `timed_out`、
//! `too_large`，`by`、`cause` 照起它的那次调用。看时间的线程等的是「结束了」那一声，结束了当场走，不轮询。

use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use miyu_kernel::event::{JobReason, JobReported};
use miyu_kernel::id::{CommandId, JobId};
use miyu_kernel::origin::By;
use miyu_store::jobs::{create_output, output_path};
use miyu_tool::{Asking, Background, Exit, JobError, JobPort, Listed, Output as Read, Process};

use super::JobLimits;
use super::output::Output;
use super::stop::{Who, Why};
use super::{Ended, Entry, Key, Shared};
use crate::TARGET;
use crate::clock::Clock;
use crate::lines::millis;

/// 命令退出以后，读输出的线程最多再等多久：还有东西拿着管道的（Windows 上它放出去的孙进程），不等它，和前台一样。
const DRAIN: Duration = Duration::from_millis(500);

/// 一次调用的任务端口：它起的命令自己退出了，照 `by`、`cause` 报；她用它停掉的，也照这两样报（施工 7-4）。
pub(super) struct Port {
    shared: Arc<Shared>,
    by: By,
    cause: Option<CommandId>,
    limits: JobLimits,
}

impl Port {
    pub(super) fn new(
        shared: Arc<Shared>,
        by: By,
        cause: Option<CommandId>,
        limits: JobLimits,
    ) -> Port {
        Port {
            shared,
            by,
            cause,
            limits,
        }
    }
}

impl JobPort for Port {
    fn start(&self, command: Background) -> io::Result<JobId> {
        let Background { output, process } = command;
        let process: Arc<dyn Process> = Arc::from(process);
        let shared = &self.shared;
        let job = shared.ids.next();
        let file = match create_output(&shared.dir, &job) {
            Ok(file) => file,
            Err(error) => {
                discard(process);
                return Err(error);
            }
        };
        let sink = Arc::new(Output::new(
            output_path(&shared.dir, &job),
            file,
            Arc::clone(&shared.changes),
        ));
        let key = (shared.owner, job.clone());
        let started = Instant::now();
        {
            let mut table = shared.table.lock();
            if !shared.open.load(Ordering::Acquire) {
                drop(table);
                discard(process);
                return Err(io::Error::other("the session stopped"));
            }
            table.insert(
                key.clone(),
                Entry {
                    process: Arc::clone(&process),
                    output: Arc::clone(&sink),
                    started,
                    reported: false,
                },
            );
        }
        // 三个线程都带着派活时的 span：它们发的行也有会话编号。
        let span = tracing::Span::current();
        let halt = Halt {
            shared: Arc::clone(shared),
            job: job.clone(),
            by: self.by.clone(),
            cause: self.cause.clone(),
        };
        let (drained, read) = mpsc::channel();
        let writer = Arc::clone(&sink);
        let reading = span.clone();
        let (limit, capping) = (self.limits.output, halt.clone());
        thread::spawn(move || {
            let _entered = reading.enter();
            let mut over = false;
            for text in output {
                writer.write(&text);
                if !over && writer.written() > limit {
                    over = true;
                    capping.halt(Why::TooLarge);
                }
            }
            #[expect(
                clippy::let_underscore_must_use,
                reason = "等的那一头不等了（已经报了），没人收也不要紧"
            )]
            let _ = drained.send(());
        });
        let (finished, done) = mpsc::channel();
        let watch = Watch {
            shared: Arc::clone(shared),
            key,
            by: self.by.clone(),
            cause: self.cause.clone(),
            started,
            _finished: finished,
        };
        let timing = span.clone();
        thread::spawn(move || {
            let _entered = span.enter();
            watch.run(&*process, &sink, &read);
        });
        let running = self.limits.running;
        thread::spawn(move || {
            let _entered = timing.enter();
            if let Err(RecvTimeoutError::Timeout) = done.recv_timeout(running) {
                halt.halt(Why::TimedOut);
            }
        });
        Ok(job)
    }

    fn list(&self) -> Vec<Listed> {
        self.shared.roster().list(Clock::default().now())
    }

    fn output(&self, job: JobId) -> Asking<'_, Result<Read, JobError>> {
        Box::pin(self.shared.output(job))
    }

    /// 她用 `jobs` 停的：`by` 是这次调用，`cause` 是它所在那一轮的，带 `by_model`，不叫醒她（施工 7-4）。
    fn stop(&self, job: JobId) -> Asking<'_, Result<(), JobError>> {
        let who = Who {
            by: self.by.clone(),
            cause: self.cause.clone(),
            why: Why::Stopped { by_model: true },
        };
        Box::pin(self.shared.stop(job, who))
    }
}

/// 等一条命令结束的那一头要的。
struct Watch {
    shared: Arc<Shared>,
    key: Key,
    by: By,
    cause: Option<CommandId>,
    started: Instant,
    /// 报完了跟着丢掉：看时间的线程就知道结束了（施工 7-12 下）。
    _finished: Sender<()>,
}

/// 到了上限停掉一条命令（施工 7-12 下）：照停掉的规矩，`by`、`cause` 照起它的那次调用。已经报过的（自己退出了、被停了）
/// 什么都不做。杀进程、碰磁盘，在自己的线程里调。
#[derive(Clone)]
struct Halt {
    shared: Arc<Shared>,
    job: JobId,
    by: By,
    cause: Option<CommandId>,
}

impl Halt {
    fn halt(&self, why: Why) {
        let who = Who {
            by: self.by.clone(),
            cause: self.cause.clone(),
            why,
        };
        if let Some(ended) = self.shared.stop_command(self.job.clone(), &who) {
            tracing::info!(target: TARGET, job = self.job.to_string().as_str(), reason = ?why, "job over its limit");
            self.shared.back(ended);
        }
    }
}

impl Watch {
    /// 等它结束；读输出的线程读完（最多再等 [`DRAIN`]），关上输出、存成 blob，报给 actor。它被停掉了、已经有人报了的，
    /// 表里不收（[`Shared::report`]）。
    fn run(self, process: &dyn Process, sink: &Output, read: &Receiver<()>) {
        let exit = process.wait();
        let took = self.started.elapsed();
        if read.recv_timeout(DRAIN).is_err() {
            tracing::debug!(target: TARGET, "job output still open after the command ended");
        }
        sink.close();
        let super::output::Stored {
            output,
            chars,
            tail,
        } = sink.stored(&self.shared.blobs);
        let (exit_code, signal) = match exit {
            Ok(Exit::Code(code)) => (Some(code), None),
            Ok(Exit::Signal(signal)) => (None, Some(signal)),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "job not waited");
                (None, None)
            }
        };
        let reported = JobReported {
            job: self.key.1.clone(),
            reason: JobReason::Exited,
            exit_code,
            signal,
            by_model: false,
            duration_ms: Some(millis(took)),
            output,
            chars,
            tail,
        };
        self.shared.report(Ended {
            key: self.key,
            by: self.by,
            cause: self.cause,
            reported,
        });
    }
}

/// 收不下的命令：整组杀掉，另起一个线程等它，免得留下没人收的僵尸进程。
fn discard(process: Arc<dyn Process>) {
    process.kill();
    thread::spawn(move || {
        if let Err(error) = process.wait() {
            tracing::debug!(target: TARGET, error = %error, "discarded job not waited");
        }
    });
}
