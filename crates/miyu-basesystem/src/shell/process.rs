//! 起命令、读输出、等它结束（施工 4-8）：标准输入接空的（脚本的参数另给一份，施工 X-12 上），标准输出、标准错误接到
//! 同一根管道上，照写出来的先后；到时、叫停时整组杀掉。
//!
//! - Unix 上命令自成一个进程组，用 `rustix` 的安全接口 `kill(-pgid, SIGKILL)` 杀整组。命令退出以后，组里还在跑的
//!   （`&` 放到后台的）也杀掉：命令做完了，它起的东西不留。后台命令（施工 7-3）照同一套起、同一套杀，见
//!   `background.rs`。
//! - 核心崩了，命令跟着没（施工 7-8，`miyu_sandbox::lifeline`）：Unix 上组里先起一个看门的，读到核心的生命线断了就杀整组，
//!   组号是看门的那一个；起不来的照旧自成一组，记一行 `WARN`。Windows 上核心自己在作业对象里，这里什么都不用做。
//! - Windows 上用系统自带的 `taskkill /T /F` 杀整棵进程树，只在命令还在跑的时候杀：进程编号回收得快，退出以后再
//!   按编号杀可能杀错。
//! - 命令退出以后，管道最多再读 [`DRAIN`]：还有东西拿着管道的（Windows 上它放出去的孙进程），不等它。
//! - 到时杀了以后最多再等 [`KILL_WAIT`]：还不结束的也算超时，不再等它（施工 4-9 再补二）。
//! - 前台到时也可以不杀、交出去接着跑（施工 7-12，[`Started::wait_or_move`]）：读到的那一段和进程交回去，读的线程把
//!   后面的输出改送进一根通道，见 `moved.rs`。

use std::io::{self, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use super::output::{Capture, Decoder};

mod group;
mod waits;

pub(super) use group::{Group, Guard};
pub(super) use waits::{Exited, Wakes};

/// 命令退出以后，管道最多再读多久。
const DRAIN: Duration = Duration::from_millis(500);

/// 到时杀了以后，最多再等多久：还不结束的（Windows 上 `taskkill` 没杀掉之类）不再等它（施工 4-9 再补二）。
const KILL_WAIT: Duration = Duration::from_secs(5);

/// 运行日志的来处：工具的名字（`28-运行日志.md` 第二节；施工 4-9 再补四上：原来写 `miyu::basesystem`，10 个字，
/// 来源那一列对不齐）。
const TARGET: &str = "miyu::shell";

/// Windows 上起的程序不弹控制台窗口。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 一条命令怎么结束的。
#[derive(Debug)]
pub(super) enum Ending {
    /// 自己结束了：退出码，或者（Unix）杀掉它的信号。
    Exited(ExitStatus),
    /// 到时了，整组杀掉了。
    TimedOut,
}

/// 跑完的一条命令：怎么结束的，读到的输出。
#[derive(Debug)]
pub(super) struct Finished {
    /// 怎么结束的。
    pub(super) ending: Ending,
    /// 读到的输出。
    pub(super) output: Capture,
}

/// 等到的：结束了，或者到时交出去接着跑（施工 7-12）。
pub(super) enum Waited {
    /// 结束了，或者到时杀掉了。
    Finished(Finished),
    /// 到时没杀，交出去接着跑。
    Moved(Handover),
}

/// 到时交出去的一条命令（施工 7-12）：到这一刻读到的、之后的输出、进程。
pub(super) struct Handover {
    /// 到这一刻读到的。
    pub(super) output: Capture,
    /// 之后的输出：读的线程解好一段送一段，读完了关掉。
    pub(super) rest: mpsc::Receiver<String>,
    /// 进程：等它结束、整组杀。
    pub(super) process: super::moved::Moved,
}

/// 读到的输出往哪送：交出去以前进内存（[`Capture`]）、推给头；交出去以后送进通道。
#[derive(Default)]
struct Sink {
    capture: Capture,
    moved: Option<mpsc::Sender<String>>,
}

/// 起好的一条命令：它的进程、整组杀的时候认它的那个编号、读输出的线程、看门的。
pub(super) struct Started {
    child: Child,
    group: Group,
    output: Arc<Mutex<Sink>>,
    read: mpsc::Receiver<()>,
    /// 等完了、整组杀过了才丢掉：丢掉时收掉它（施工 7-8）。
    _watch: Watch,
}

/// 组里看门的（施工 7-8，`miyu_sandbox::lifeline`）：Unix 上一个组一个，丢掉时收掉；Windows 上核心自己在作业对象里，这里
/// 拿着这条命令自己的作业对象（施工 M-3 下，只用来认人）。
pub(super) struct Watch {
    /// 看门的：起不来的没有。
    #[cfg(unix)]
    watcher: Option<miyu_sandbox::lifeline::Watcher>,
    /// 这条命令的作业对象：建不成的没有。
    #[cfg(windows)]
    job: Option<miyu_sandbox::lifeline::Job>,
}

/// 脚本调工具的通道认人时照它（施工 M-3）：Unix 上是命令的进程组号，Windows 上是命令的作业对象。
#[cfg(unix)]
pub(in crate::shell) type Member = u32;

/// 同上：Windows 上是命令的作业对象。
#[cfg(windows)]
pub(in crate::shell) type Member = miyu_sandbox::lifeline::Job;

/// 同上：别的系统没有通道。
#[cfg(not(any(unix, windows)))]
pub(in crate::shell) type Member = ();

/// 一条命令的 [`Member`]：Windows 上作业对象建不成的没有。
#[cfg(unix)]
pub(super) fn member(group: Group, _watch: &Watch) -> Option<Member> {
    Some(group.id())
}

/// 同上：Windows 上是 `watch` 拿着的作业对象。
#[cfg(windows)]
pub(super) fn member(_group: Group, watch: &Watch) -> Option<Member> {
    watch.job.clone()
}

/// 同上：别的系统没有。
#[cfg(not(any(unix, windows)))]
pub(super) fn member(_group: Group, _watch: &Watch) -> Option<Member> {
    None
}

/// 起好的一条命令的几样：进程、整组杀时认的编号、管道读的一头、看门的。
pub(super) struct Spawned {
    pub(super) child: Child,
    pub(super) group: Group,
    pub(super) pipe: io::PipeReader,
    pub(super) watch: Watch,
}

/// 起命令：标准输入接空的，有 `input` 的（脚本的参数，施工 X-12 上）接一根管道、写完就关；标准输出、标准错误接到同一根
/// 管道上，自成一组（Unix 上组里有看门的，施工 7-8）。交回它的进程、整组杀的时候认它的那个编号、管道读的一头、看门的。
/// 前台、后台（施工 7-3）都这样起。
///
/// # Errors
///
/// 管道建不起来，或者程序起不来（找不到、工作目录不在）。
pub(super) fn spawn(mut command: Command, input: Option<Vec<u8>>) -> io::Result<Spawned> {
    let (pipe, writer) = io::pipe()?;
    let stdin = match input {
        Some(_) => Stdio::piped(),
        None => Stdio::null(),
    };
    command
        .stdin(stdin)
        .stdout(writer.try_clone()?)
        .stderr(writer);
    let mut watch = alone(&mut command);
    // 父进程手里的写端在 `command` 里，这个函数一返回就跟着它关掉：留着的话，命令都退出了读的一头也读不到结尾。
    let mut child = command.spawn()?;
    joined(&mut watch, &child);
    // 另起一个线程写完就关：在这里写，它不读的话管道满了会卡住起它的这一头。
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        thread::spawn(move || {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "不读标准输入的脚本早早关了管道，写不进也不要紧"
            )]
            let _ = stdin.write_all(&bytes);
        });
    }
    let group = Group(group_of(&watch, &child));
    Ok(Spawned {
        child,
        group,
        pipe,
        watch,
    })
}

/// 整组杀时认的编号：Unix 上有看门的是它的组，没有的是命令自己的组；Windows 上是命令的进程号（杀整棵树）。
#[cfg(unix)]
fn group_of(watch: &Watch, child: &Child) -> u32 {
    watch
        .watcher
        .as_ref()
        .map_or_else(|| child.id(), miyu_sandbox::lifeline::Watcher::group)
}

/// 同上：别的平台是命令的进程号。
#[cfg(not(unix))]
fn group_of(_watch: &Watch, child: &Child) -> u32 {
    child.id()
}

/// 起前台的命令：读到的一段段解成字交给 `progress`。`progress_lines` 说结果里去不去掉进度行（施工 X-45，只有脚本工具去掉）。
///
/// # Errors
///
/// 同 [`spawn`]。
pub(super) fn start(
    command: Command,
    input: Option<Vec<u8>>,
    progress: impl Fn(String) + Send + 'static,
    progress_lines: bool,
) -> io::Result<Started> {
    let Spawned {
        child,
        group,
        pipe,
        watch,
    } = spawn(command, input)?;
    let capture = Capture::new(progress_lines);
    let output = Arc::new(Mutex::new(Sink {
        capture,
        ..Sink::default()
    }));
    let (done, read) = mpsc::channel();
    let theirs = Arc::clone(&output);
    // 读的线程带着派活时的 span：它发的行也有会话编号（施工 4-9 再补四上）。
    let span = tracing::Span::current();
    thread::spawn(move || {
        let _entered = span.enter();
        pump(pipe, &theirs, &progress);
        #[expect(
            clippy::let_underscore_must_use,
            reason = "等的那一头可能已经不等了，没人收也不要紧"
        )]
        let _ = done.send(());
    });
    Ok(Started {
        child,
        group,
        output,
        read,
        _watch: watch,
    })
}

impl Started {
    /// 整组杀的时候认它。
    pub(super) fn group(&self) -> Group {
        self.group
    }

    /// 脚本调工具的通道认人时照它（施工 M-3）。
    pub(super) fn member(&self) -> Option<Member> {
        member(self.group, &self._watch)
    }
}

/// 结束了：组里剩下的杀掉，放下句柄，管道最多再读 [`DRAIN`]，交回读到的。
fn done(
    ending: Ending,
    child: Option<Child>,
    group: Group,
    output: &Mutex<Sink>,
    read: &mpsc::Receiver<()>,
) -> Finished {
    group.leftovers();
    drop(child);
    if read.recv_timeout(DRAIN).is_err() {
        tracing::debug!(target: TARGET, "command output still open after the command ended");
    }
    let output = std::mem::take(&mut lock(output).capture).finished();
    Finished { ending, output }
}

/// 读管道，直到读完或者读不了。交出去以前进内存、推给头；交出去以后解好的一段段送进通道（施工 7-12）。读完了这个线程
/// 一退出、等的那一头早就放下了 [`Sink`]，留在里面的那一头跟着丢掉，通道就关了。
fn pump(mut pipe: io::PipeReader, output: &Mutex<Sink>, progress: &impl Fn(String)) {
    let mut decoder = Decoder::default();
    let mut buffer = [0u8; 8192];
    let mut moved = None;
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                let chunk = &buffer[..count];
                let text = decoder.push(chunk);
                if moved.is_none() {
                    let mut sink = lock(output);
                    match &sink.moved {
                        Some(sender) => moved = Some(sender.clone()),
                        None => sink.capture.push(chunk),
                    }
                }
                match &moved {
                    Some(sender) => send(sender, text),
                    None if !text.is_empty() => progress(text),
                    None => {}
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                tracing::debug!(target: TARGET, error = %error, "command output not readable");
                break;
            }
        }
    }
    // 读完以后才交出去的：最后半个字已经进了内存，在交出去的那一段里，不再送。一个字正好切在交出去那一刻两边的，那个字
    // 可能成了 `\u{FFFD}`。
    let rest = decoder.finish();
    match moved {
        Some(sender) => send(&sender, rest),
        None if !rest.is_empty() => progress(rest),
        None => {}
    }
}

/// 交出去以后的一段送进通道：空的不送；收的那一头不要了也不要紧。
fn send(sender: &mpsc::Sender<String>, text: String) {
    if !text.is_empty() {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "任务表不读了（停了、核心在停），读到的就没人要了"
        )]
        let _ = sender.send(text);
    }
}

/// 读到的输出：读的线程 panic 了也照样拿。
fn lock(output: &Mutex<Sink>) -> MutexGuard<'_, Sink> {
    output.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 让命令自成一组，整组杀得掉：Unix 上先起看门的，命令进它的组（施工 7-8），起不来的照旧自成一组。Windows 上不弹控制台
/// 窗口。
#[cfg(unix)]
fn alone(command: &mut Command) -> Watch {
    use std::os::unix::process::CommandExt;
    let watcher = match miyu_sandbox::lifeline::watcher() {
        Ok(watcher) => match i32::try_from(watcher.group()) {
            Ok(group) => {
                command.process_group(group);
                Some(watcher)
            }
            Err(_) => {
                command.process_group(0);
                None
            }
        },
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "command lifeline not started");
            command.process_group(0);
            None
        }
    };
    Watch { watcher }
}

/// 同上：Windows 上不弹控制台窗口。
#[cfg(windows)]
fn alone(command: &mut Command) -> Watch {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
    Watch { job: None }
}

/// 命令起来了，马上放进它自己的作业对象（Windows，施工 M-3 下）：放不进去的记一行，命令照旧跑，只是通道认不出它的进程。
#[cfg(windows)]
fn joined(watch: &mut Watch, child: &Child) {
    match miyu_sandbox::lifeline::Job::for_child(child) {
        Ok(job) => watch.job = Some(job),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "command job object not set up")
        }
    }
}

/// 同上：别的系统不用。
#[cfg(not(windows))]
fn joined(_watch: &mut Watch, _child: &Child) {}

/// 同上：别的系统什么都不做。
#[cfg(not(any(unix, windows)))]
fn alone(_command: &mut Command) -> Watch {
    Watch {}
}

#[cfg(all(test, unix))]
mod tests;
