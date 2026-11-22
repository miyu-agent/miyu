//! 发命令（施工 3-7 中）：发一个、等它的回应。人敲的斜杠命令可以连人敲的那一行一起交（施工 FX-15）。

use tokio::sync::oneshot;

use miyu_kernel::event::TypedCommand;
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};

use super::{Handle, Message, Stopped};

impl Handle {
    /// 发一个命令，等它的回应：接受的，它产生的事件落了盘才回（`07-存储.md` S4）；拒绝的当场回。
    /// 编号 `id` 由发命令的一方生成，同一个编号只生效一次（`02-内核.md` 不变量 9）。
    ///
    /// # Errors
    ///
    /// 会话停了：写不进去、出了 bug、有计划地停下了。
    pub async fn command(
        &self,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Result<Outcome, Stopped> {
        self.sent(id, by, command, None).await
    }

    /// 同 [`Handle::command`]，连人敲的那一行 `typed` 一起交（施工 FX-15，`internal/blueprint/goal.md`「在新的架构里怎么放」
    /// 第 2 条）：`/goal <目标>` 交的设下、接着跑，由它开的续轮的第一轮带上这一行（`message.user` 的 `command`）。内核不看它。
    ///
    /// # Errors
    ///
    /// 同 [`Handle::command`]。
    pub async fn command_typed(
        &self,
        id: CommandId,
        by: By,
        command: Command,
        typed: TypedCommand,
    ) -> Result<Outcome, Stopped> {
        self.sent(id, by, command, Some(typed)).await
    }

    async fn sent(
        &self,
        id: CommandId,
        by: By,
        command: Command,
        typed: Option<TypedCommand>,
    ) -> Result<Outcome, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Command {
            id,
            by,
            command: Box::new(command),
            typed,
            reply,
        })?;
        answer.await.map_err(|_| Stopped)
    }
}
