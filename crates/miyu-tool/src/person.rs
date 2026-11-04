//! 人在头的输入框里用 `!` 跑的一条命令（施工 SH-1 上，设计 `57-人在会话里跑命令.md` 第二节第 1 条）：声明跑命令的工具
//! （[`crate::Tool::runs_commands`]）多一个可选的钩子 [`crate::Tool::run_for_person`]，交回结构化的结果。
//!
//! **只给人用**：核心只在收到人发的 `session.send {shell: true}` 时调它（别的 agent、扩展、场所会话在协议那一层就拒了）；她的
//! 工具调用走 [`crate::Tool::run`]，碰不到这里。这条路不判权限、不进沙盒、不过 `shell` 给她设的几道挡（高危、干等、到时挪到
//! 后台、`miyu call` 的通道）：人亲手打的就是许可（2026-11-03 项目主人当面定）。

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

/// 人打的一条命令：原样的命令行，在哪个目录里跑（会话这一刻的工作目录，`~` 已经接好）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonCommand {
    /// 命令行，原样。
    pub command: String,
    /// 在哪个目录里跑。
    pub cwd: PathBuf,
}

/// 跑完的样子（设计 57 第四节第 2 条的 `shell.ran`）：怎么结束的，退出码或信号，合法化过的输出（标准输出、标准错误合在一起，
/// `\r\n` 换成 `\n`，太长的照前台的截法留头尾），截掉几个字，用了多久。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRun {
    /// 怎么结束的。
    pub ended: PersonEnd,
    /// 退出码；被信号杀掉的、到时、被打断的没有。
    pub exit: Option<i32>,
    /// 杀掉它的信号（Unix）。
    pub signal: Option<u32>,
    /// 输出，截过的中间有一行写省了多少。
    pub text: String,
    /// 截掉了几个字（Unicode 字符），没截的是 0。
    pub omitted: u64,
    /// 从起到结束的毫秒数。
    pub duration_ms: u64,
}

/// 一条人打的命令怎么结束的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonEnd {
    /// 自己退出了，被信号杀掉的也算。
    Exited,
    /// 跑到了时限，整组杀掉了。
    TimedOut,
    /// 人打断了，整组杀掉了。
    Interrupted,
}

/// 跑着的一条：等它交回结果。起不来（程序找不到、工作目录不在）的交回出错。
pub type PersonRunning<'a> = Pin<Box<dyn Future<Output = io::Result<PersonRun>> + Send + 'a>>;

/// 叫停的口子：核心打断时举起来（[`Halt::halt`]），跑命令的工具事先登记的那一下（[`Halt::on_halt`]）当场被调；举起来时还没
/// 登记的，登记的那一刻就调。不轮询。
#[derive(Clone, Default)]
pub struct Halt(Arc<Mutex<Raised>>);

/// 举没举起来、举起来以后调哪一下。
#[derive(Default)]
struct Raised {
    raised: bool,
    then: Option<Box<dyn FnOnce() + Send>>,
}

impl Halt {
    /// 一个还没举起来的。
    pub fn new() -> Halt {
        Halt::default()
    }

    /// 举起来：登记过的那一下当场调，之后再登记的也马上调。举过的再举不再调。
    pub fn halt(&self) {
        let then = {
            let mut raised = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            raised.raised = true;
            raised.then.take()
        };
        if let Some(then) = then {
            then();
        }
    }

    /// 登记举起来时调的那一下：已经举起来的当场调。
    pub fn on_halt(&self, then: impl FnOnce() + Send + 'static) {
        let mut raised = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if raised.raised {
            drop(raised);
            then();
            return;
        }
        raised.then = Some(Box::new(then));
    }
}

impl std::fmt::Debug for Halt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raised = self.0.lock().unwrap_or_else(PoisonError::into_inner).raised;
        f.debug_tuple("Halt").field(&raised).finish()
    }
}

#[cfg(test)]
mod tests;
