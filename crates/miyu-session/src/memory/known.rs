//! 认出来（施工 E-1 补，`docs/blueprint/memory.md` 第三条第 8 款，设计 `42-认识的人.md` PN4）：一轮开始时，和她说话的那个人
//! （这一轮开口的人里最后开口的、不是主人的那个，施工 E-0 的 `speakers`）有人卡的，把他的卡带上。
//!
//! - 一轮最多一张；这一段上下文里这个人的卡带过的不再带（照 `present` 里模块 `memory`、类别 `card` 的 `refs`），压缩以后
//!   重新算。主人不带（他的事在常驻摘要里），没有卡的、卡作废了的不带。
//! - 一块 `context.injected`，模块 `memory`、类别 `card`，`refs` 是他的平台编号；排在常驻摘要后面、联想前面，在这一轮的话
//!   前面，以前的不动（缓存不断）。卡最多 400 字（整理、人改的时候已经截了；设计 42 PN4）。

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::{Present, Speaker};
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_recall::PersonCard;
use miyu_tool::load;

use super::Keeper;

/// 注入那一块的模块：和常驻那一块同一个 `memory`。
const MODULE: &str = "memory";

/// 注入那一块的类别：`card`。
const KIND: &str = "card";

impl Keeper {
    /// 这一轮带谁的卡（`speakers` 是这一轮开口的人，最近开口的在后；`present` 是内核交来的这一段上下文里模块注入过的）。人格
    /// 记忆没装的、核心没读到外壳的字的、读不了的没有。碰磁盘，调的一方放在阻塞线程里。
    pub(crate) fn card(&self, speakers: &[Speaker], present: &[Present]) -> Option<Injection> {
        if !self.installed() {
            return None;
        }
        let texts = self.memory.summary.as_ref()?;
        let who = talking(speakers)?;
        if brought(present, who) {
            return None;
        }
        let card = match self.log() {
            Ok(log) => log.book(|book| {
                book.card(who)
                    .filter(|card| card.retired.is_none())
                    .cloned()
            }),
            Err(refused) => {
                let error = format!("{refused:?}");
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory card not read");
                return None;
            }
        }?;
        Some(render(&texts.card, &card))
    }
}

/// 和她说话的那个人：开口的人里最后一个不是主人的。
fn talking(speakers: &[Speaker]) -> Option<&str> {
    speakers
        .iter()
        .rev()
        .find(|speaker| !speaker.owner)
        .map(|speaker| speaker.id.as_str())
}

/// 这一段上下文里 `who` 的卡带过了。
fn brought(present: &[Present], who: &str) -> bool {
    present.iter().any(|present| {
        present.module.as_str() == MODULE
            && present.kind.as_str() == KIND
            && present.refs.iter().any(|seen| seen == who)
    })
}

/// 拼那一块：是谁（有叫法的接在后面）、卡的正文。
fn render(template: &miyu_kernel::template::Template, card: &PersonCard) -> Injection {
    let who = match card.names.is_empty() {
        true => card.who.clone(),
        false => format!("{} ({})", card.who, card.names.join(", ")),
    };
    let text = card.text.replace('\n', " ");
    let said = load::say(template, &[("who", &who), ("text", &text)]);
    Injection {
        module: ModuleId::parse(MODULE).expect("合写法"),
        fact: ContextInjected {
            kind: FactKind::parse(KIND).expect("合写法"),
            text: format!("{}\n", said.trim_end()),
            refs: vec![card.who.clone()],
            after: false,
        },
    }
}

#[cfg(test)]
mod tests;
