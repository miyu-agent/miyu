//! 回合开始的挂接点每一段等了多久（施工 PF-3，`docs/blueprint/session/actor.md`「运行日志」的 `turn start timed`）：CPU 被抢
//! 时回合开始 93 毫秒到 5.5 秒（施工 PF-2 第 4 条），日志里分不出慢在哪一跳。`actor/model.rs` 的 `turn_start` 串着冻结配置
//! → 刷新人格和找技能（并着）→ 压缩线 → 她现在在干嘛（施工 LF-7）→ 记忆、知识库、扩展、项目指令一起等最慢的那一个，每一段用
//! [`Lap::run`] 套着量，回合开始的输入交给内核之前记一行（[`TurnStart::log`]）。
//!
//! 每一段还量它里面 `crate::blocking::blocking` 来回等的：交给阻塞线程到那边开始做，加上那边做完到这一头接着跑，不算那边
//! 做活的时间（[`Waited`]）。被抢 CPU 时一跳排队多久就看这一格。只算这一段自己等的 `blocking`：派到别的任务里的不算。
//!
//! 只在这个来源调到 `DEBUG` 时量（照 YC-14 的 `request timed`，`super::on`）：平常每段只多问一回「有没有人听」，不读时钟、
//! 不套 task-local。量的都是用时，没有对话的字。

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use miyu_kernel::id::TurnId;

use super::{micros, on};
use crate::TARGET;

tokio::task_local! {
    /// [`Lap::run`] 套着的那一段里，`blocking` 来回等的攒在这里。
    static WAITED: Waited;
}

/// 一段里 `blocking` 来回等了多少微秒，攒着的。
#[derive(Debug, Clone, Default)]
pub(crate) struct Waited(Arc<AtomicU64>);

impl Waited {
    /// 正在量的那一段的：`blocking` 里问，没在量的（`DEBUG` 关着、不在回合开始）没有。
    pub(crate) fn current() -> Option<Waited> {
        WAITED.try_with(Waited::clone).ok()
    }

    /// 又等了 `waited`。
    pub(crate) fn add(&self, waited: Duration) {
        let us = u64::try_from(waited.as_micros()).unwrap_or(u64::MAX);
        self.0.fetch_add(us, Ordering::Relaxed);
    }

    fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// 一段：整段用了多少微秒，里面 `blocking` 来回等了多少。没跑过的（`DEBUG` 关着、这一轮没问这一件）都没有，那一行里不写。
#[derive(Debug, Default)]
pub(crate) struct Lap {
    us: Option<u64>,
    wait_us: Option<u64>,
}

impl Lap {
    /// 等 `work` 做完，量着：跑几回的（人格换了照新快照再找一次技能）加在一起。
    pub(crate) async fn run<T>(&mut self, work: impl Future<Output = T>) -> T {
        if !on() {
            return work.await;
        }
        let waited = Waited::default();
        let started = Instant::now();
        let done = WAITED.scope(waited.clone(), work).await;
        add(&mut self.us, micros(started));
        add(&mut self.wait_us, waited.get());
        done
    }
}

fn add(slot: &mut Option<u64>, us: u64) {
    *slot = Some(slot.unwrap_or(0).saturating_add(us));
}

/// 记忆那个挂接点里的四件（`actor/model/hooks.rs` 的 `remember`）：摘要和卡并着，再新记的（施工 LF-6），再联想。
#[derive(Debug, Default)]
pub(crate) struct Remember {
    /// 常驻的摘要。
    pub(crate) summary: Lap,
    /// 和她说话的那个人的卡。
    pub(crate) card: Lap,
    /// 人生循环叫醒时新记的（施工 LF-6）；别的回合一问就回，也量。
    pub(crate) fresh: Lap,
    /// 联想。
    pub(crate) recall: Lap,
}

/// 记忆、知识库两个挂接点（`actor/model/hooks.rs` 的 `hooks`）。
#[derive(Debug, Default)]
pub(crate) struct Hooks {
    /// 知识库自动想起。
    pub(crate) knowledge: Lap,
    /// 记忆整个那一件：摘要、卡、新记的、联想。
    pub(crate) memory: Lap,
    /// 记忆里的四件。
    pub(crate) remember: Remember,
}

/// 一次回合开始的几段。
#[derive(Debug, Default)]
pub(crate) struct TurnStart {
    /// 从哪一刻算起；`DEBUG` 关着的没有，不记那一行。
    started: Option<Instant>,
    /// 冻结这一轮的配置。
    pub(crate) config: Lap,
    /// 刷新人格（和找技能并着）。
    pub(crate) persona: Lap,
    /// 找技能；人格换了的再找一次，加在一起。
    pub(crate) skills: Lap,
    /// 压缩触发线。
    pub(crate) line: Lap,
    /// 同一个人格的人生里她现在在干嘛（施工 LF-7，`actor/model/doing.rs`）：只查内存。
    pub(crate) doing: Lap,
    /// 记忆、知识库两个挂接点。
    pub(crate) hooks: Hooks,
    /// 问扩展。
    pub(crate) extensions: Lap,
    /// 项目指令。
    pub(crate) project: Lap,
}

impl TurnStart {
    /// 回合开始了：`DEBUG` 开着的从这一刻算。
    pub(crate) fn start() -> TurnStart {
        TurnStart {
            started: on().then(Instant::now),
            ..TurnStart::default()
        }
    }

    /// 回合 `turn` 的几段都等完了，这就交给内核：记那一行。
    pub(crate) fn log(self, turn: TurnId) {
        let Some(started) = self.started else {
            return;
        };
        let Hooks {
            knowledge,
            memory,
            remember,
        } = self.hooks;
        tracing::debug!(
            target: TARGET,
            turn = turn.started().get(),
            config_us = self.config.us,
            config_wait_us = self.config.wait_us,
            persona_us = self.persona.us,
            persona_wait_us = self.persona.wait_us,
            skills_us = self.skills.us,
            skills_wait_us = self.skills.wait_us,
            line_us = self.line.us,
            line_wait_us = self.line.wait_us,
            doing_us = self.doing.us,
            memory_us = memory.us,
            summary_us = remember.summary.us,
            summary_wait_us = remember.summary.wait_us,
            card_us = remember.card.us,
            card_wait_us = remember.card.wait_us,
            fresh_us = remember.fresh.us,
            fresh_wait_us = remember.fresh.wait_us,
            recall_us = remember.recall.us,
            recall_wait_us = remember.recall.wait_us,
            knowledge_us = knowledge.us,
            knowledge_wait_us = knowledge.wait_us,
            extensions_us = self.extensions.us,
            project_us = self.project.us,
            project_wait_us = self.project.wait_us,
            total_us = micros(started),
            "turn start timed"
        );
    }
}
