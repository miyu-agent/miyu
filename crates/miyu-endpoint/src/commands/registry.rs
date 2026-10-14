//! 斜杠命令的登记表（施工 O-6 三补，`docs/blueprint/protocol.md`「`command.run`」第 11 条）：一条命令一个 [`Slash`]，各自
//! 在管它的模块里写，起来时登记进 [`Commands`]。`command.run` 照它认、判、执行，`command.catalog` 照它列；中心不再写名字。
//! 以前是一张写死的枚举，记忆、长期目标要加一条命令都得改中心（记忆的会话提，2026-10-14）。
//!
//! 名字和别名在同一个名字空间里，照原样比（命令名都是小写）。核心自己的先登记；内置模块之间撞名是写错了，登记时就 `panic`，
//! 测试照全部内置命令登记一遍当场拦（`registry/tests.rs`）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_session::Handle;

use super::Said;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::sessions::Found;

/// 执行一条命令时手里有的。
pub(crate) struct Call<'a> {
    /// 核心。
    pub(crate) core: &'a Arc<Core>,
    /// 发命令的连接：回执、用不了的原因照它的语言写。
    pub(crate) peer: &'a Peer,
    /// 哪个会话。
    pub(crate) session: &'a SessionId,
    /// 找到的会话：句柄、工作目录。
    pub(crate) found: &'a Found,
    /// 请求的编号：交给内核的命令照它只生效一次。
    pub(crate) id: &'a CommandId,
    /// 谁打的。
    pub(crate) by: &'a By,
    /// 命令名后面跟的字，去掉了前后空白。
    pub(crate) rest: &'a str,
    /// 头所在的目录（`command.run` 的 `cwd`）。
    pub(crate) head: Option<&'a str>,
}

impl Call<'_> {
    /// 会话的句柄。
    pub(crate) fn handle(&self) -> &Handle {
        &self.found.handle
    }
}

/// 执行完了：这一次追加的事件的序号，和回执。
pub(crate) struct Ran {
    /// 追加的事件的序号（`command.ran` 另记，不在这里）。
    pub(crate) events: Vec<u64>,
    /// 回执那一句。
    pub(crate) said: Said,
}

/// 执行的结果。
pub(crate) type Running<'a> = Pin<Box<dyn Future<Output = Result<Ran, Refusal>> + Send + 'a>>;

/// 一条斜杠命令。
pub(crate) trait Slash: Send + Sync {
    /// 正名。
    fn name(&self) -> &'static str;

    /// 别名。
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// 做什么的那一句：`core/human/<语言>.json` 的键（`command.catalog` 的 `summary`）。
    fn summary(&self) -> &'static str;

    /// 名字后面跟什么的提示，同上的键（`command.catalog` 的 `argument`）；不跟字的没有。
    fn argument(&self) -> Option<&'static str> {
        None
    }

    /// 在后台答（要等模型的，例如 `/dream`）：同一个连接后面的请求不等它。
    fn later(&self) -> bool {
        false
    }

    /// 身份那一关（`may_run`）以外，这个人在这个会话里能不能用：主人本人才行、要开着记忆这些。`command.run`、
    /// `command.catalog` 同一份判法。
    fn allowed(&self, _core: &Core, _handle: &Handle, _by: &By) -> Result<(), Refusal> {
        Ok(())
    }

    /// 执行。
    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a>;
}

/// 登记表：照名字排。
#[derive(Default)]
pub(crate) struct Commands {
    list: Vec<Arc<dyn Slash>>,
}

impl Commands {
    /// 登记一条。
    ///
    /// # Panics
    ///
    /// 名字、别名和已经登记的哪一条的名字、别名撞了：内置命令写错了，起来时就拦下。
    pub(crate) fn register(&mut self, slash: Arc<dyn Slash>) {
        for name in std::iter::once(slash.name()).chain(slash.aliases().iter().copied()) {
            if let Some(taken) = self.find(name) {
                panic!("斜杠命令 /{name} 撞名：/{} 已经占了", taken.name());
            }
        }
        let at = self.list.partition_point(|each| each.name() < slash.name());
        self.list.insert(at, slash);
    }

    /// 名字或别名是 `name` 的那一条。
    pub(crate) fn find(&self, name: &str) -> Option<&Arc<dyn Slash>> {
        self.list
            .iter()
            .find(|slash| slash.name() == name || slash.aliases().contains(&name))
    }

    /// 全部，照名字排。
    pub(crate) fn all(&self) -> impl Iterator<Item = &Arc<dyn Slash>> {
        self.list.iter()
    }
}

#[cfg(test)]
mod tests;
