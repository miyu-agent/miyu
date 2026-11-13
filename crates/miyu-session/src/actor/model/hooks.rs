//! 回合开始问记忆、知识库两个挂接点（施工 R-4 上、R-8、K-6 下；施工 PF-3 从 `model.rs` 挪出来，那边放不下了）：各自限时
//! [`HOOKS`]，每一件照 `timing/turn.rs` 的 [`Hooks`] 量着等了多久。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::facts::{Present, Speaker};
use miyu_kernel::origin::By;
use miyu_kernel::session::Injection;
use miyu_kernel::time::UtcOffset;

use crate::TARGET;
use crate::config::TurnConfig;
use crate::knowledge::Shelves;
use crate::memory::Calls;
use crate::timing::turn::{Hooks, Remember};

/// 回合开始问挂接点最多等多久（施工 R-4 上定的起点）：过了当这一轮没交。读的是本机的小库，平常几毫秒。
const HOOKS: Duration = Duration::from_millis(300);

/// 问回合开始的挂接点：记忆（施工 R-4 上，`memory.md` 第三条；`memory` 是它的端口和会话的时区）交常驻的摘要、人生循环叫醒时
/// 新记的（施工 LF-6，照开这一轮的那条消息是谁发的 `opener`）和联想（施工 R-8，第四条），知识库（施工 K-6 下，`knowledge.md`
/// 第九条；`knowledge` 是会话的知识库）交自动想起。`said` 是这一轮人说的话，不是人开的一轮没有；照意思找照这一轮的配置 `config`；`present` 是内核交来的这一段上下文里模块注入过的。两个挂接点并行问，
/// 照模块编号排（知识库在记忆前面）；各自限时 [`HOOKS`]：过了的那一件当这一轮没交，记一行 `DEBUG`（那次读在阻塞线程里照样
/// 读完，交回的丢掉）。问了的那几件量着等了多久，记在 `laps`（施工 PF-3）。
pub(super) async fn hooks(
    memory: Option<(Calls, UtcOffset)>,
    knowledge: Option<Arc<Shelves>>,
    (present, speakers, opener): (Vec<Present>, Vec<Speaker>, Option<By>),
    said: Option<String>,
    config: TurnConfig,
    laps: &mut Hooks,
) -> Vec<Injection> {
    // 拆开再借：两个并着的 future 各拿各的那几格。
    let Hooks {
        knowledge: knowing,
        memory: remembering,
        remember: parts,
    } = laps;
    let known = async {
        let (shelves, said) = (knowledge?, said.as_deref()?);
        let using = shelves.using(&config);
        knowing
            .run(timed(
                "knowledge",
                "recall",
                shelves.recall(&using, said, &present),
            ))
            .await
    };
    let remembered = async {
        let Some(memory) = memory else {
            return Vec::new();
        };
        remembering
            .run(remember(
                memory,
                (present.clone(), speakers, opener),
                said.clone(),
                Arc::clone(&config),
                parts,
            ))
            .await
    };
    let (known, remembered) = tokio::join!(known, remembered);
    known.into_iter().chain(remembered).collect()
}

/// 记忆那一个挂接点：先问摘要、和她说话的那个人的卡（施工 E-1 补，照这一轮开口的人 `speakers`），再问人生循环叫醒时新记的
/// （施工 LF-6，`opener`）、联想（这两样都要知道这一轮摘要列了哪几条：新记的从它以后算，联想不重复带）。照这个先后排：摘要、
/// 卡、新记的、联想（人生循环叫醒的一轮没有人说的话，后两样不会同一轮都有）。
async fn remember(
    (memory, offset): (Calls, UtcOffset),
    (present, speakers, opener): (Vec<Present>, Vec<Speaker>, Option<By>),
    said: Option<String>,
    config: TurnConfig,
    laps: &mut Remember,
) -> Vec<Injection> {
    let (summary, card) = tokio::join!(
        laps.summary.run(timed(
            "memory",
            "summary",
            memory.summary(offset, present.clone())
        )),
        laps.card.run(timed(
            "memory",
            "card",
            memory.card(speakers, present.clone())
        )),
    );
    let summary: Vec<Injection> = summary.into_iter().chain(card).collect();
    // 这一轮摘要列的几条，新记的、联想当已经交过的。
    let mut seen = present;
    seen.extend(summary.iter().map(|injection| Present {
        module: injection.module.clone(),
        kind: injection.fact.kind.clone(),
        refs: injection.fact.refs.clone(),
    }));
    let fresh = laps
        .fresh
        .run(timed(
            "memory",
            "fresh",
            memory.fresh(opener.as_ref(), offset, &seen),
        ))
        .await;
    let summary: Vec<Injection> = summary.into_iter().chain(fresh).collect();
    let Some(said) = said else {
        return summary;
    };
    let recall = laps
        .recall
        .run(timed(
            "memory",
            "recall",
            memory.recall(said, offset, seen, &config),
        ))
        .await;
    summary.into_iter().chain(recall).collect()
}

/// 限时 [`HOOKS`] 等挂接点的一件（`module` 是哪个挂接点，`part`：`summary`、`card`、`fresh`、`recall`）；过了的没有，记一行 `DEBUG`。
async fn timed(
    module: &'static str,
    part: &'static str,
    asked: impl Future<Output = Option<Injection>>,
) -> Option<Injection> {
    match tokio::time::timeout(HOOKS, asked).await {
        Ok(injected) => injected,
        Err(_) => {
            tracing::debug!(target: TARGET, module, part, "turn start hook timed out");
            None
        }
    }
}
