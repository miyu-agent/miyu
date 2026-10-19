//! 记忆的斜杠命令（施工 R-3 补、R-7 补，O-6 三补起由记忆这边登记）：`/remember <话>` 直接记一条记忆，不经过模型，只在本机的
//! 会话里；`/dream` 现在就整理记忆，先抽这一段、再合，回执报几样数，要等模型，在后台答；`/dream stop` 叫停这个会话正在跑的
//! 整理（施工 R-22，`/stop` 也叫停它），没在整理的回 `not_dreaming`；`/clear-memory`（别名
//! `/reset-memory`，施工 R-3 五补）清掉这个会话产生的记忆，不先问，回执说清了几条。三条都要这个会话开着记忆（`memory.md`
//! 「协议」、第七条第 9 款，`protocol.md`「`command.run`」第 8、9、12 条）。

use std::sync::Arc;

use miyu_kernel::origin::By;
use miyu_session::Handle;

use crate::Core;
use crate::commands::{Call, Commands, Ran, Running, Said, Slash};
use crate::refusal::Refusal;

/// 登记这三条。
pub(crate) fn register(commands: &mut Commands) {
    commands.register(Arc::new(Remember));
    commands.register(Arc::new(Dream));
    commands.register(Arc::new(ClearMemory));
}

/// 这个会话开着记忆的才能用。
fn remembering(core: &Core, handle: &Handle) -> Result<(), Refusal> {
    match super::remembers(core, handle) {
        true => Ok(()),
        false => Err(Refusal::MEMORY_UNAVAILABLE),
    }
}

/// 人格记忆这个包的编号（同 `miyu_memory::PACKAGE`；端点不依赖软件包的 crate）。
const PACKAGE: &str = "memory";

/// `/remember <话>`。
struct Remember;

impl Slash for Remember {
    fn name(&self) -> &'static str {
        "remember"
    }

    fn package(&self) -> &'static str {
        PACKAGE
    }

    fn summary(&self) -> &'static str {
        "commands/summary/remember"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/remember")
    }

    fn allowed(&self, core: &Core, handle: &Handle, _by: &By) -> Result<(), Refusal> {
        remembering(core, handle)
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let remembered = super::remember_in(
                call.core,
                call.handle(),
                call.id,
                call.by.clone(),
                call.rest,
            )
            .await?;
            // 回执说记下了那句话，不露编号（施工 R-3 四补）；换行这些在一行回执里换成空格，记下的原文不动。
            Ok(Ran {
                events: Vec::new(),
                said: Said {
                    key: "commands/remembered",
                    fields: vec![("text", remembered.replace(char::is_control, " "))],
                },
            })
        })
    }
}

/// `/dream`：在后台答；`/dream stop` 叫停（施工 R-22）。
struct Dream;

impl Slash for Dream {
    fn name(&self) -> &'static str {
        "dream"
    }

    fn package(&self) -> &'static str {
        PACKAGE
    }

    fn summary(&self) -> &'static str {
        "commands/summary/dream"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/dream")
    }

    fn usage(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("", "commands/usage/dream/now"),
            ("stop", "commands/usage/dream/stop"),
        ]
    }

    fn later(&self) -> bool {
        true
    }

    fn allowed(&self, core: &Core, handle: &Handle, _by: &By) -> Result<(), Refusal> {
        remembering(core, handle)
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let said = match call.rest {
                "" => {
                    let dreamed =
                        super::dream::in_session(call.core, call.session, call.handle()).await?;
                    dreamed_said(&dreamed)
                }
                "stop" => match call.handle().stop_dream().await {
                    Ok(true) => Said::plain("commands/dream-stopped"),
                    Ok(false) => return Err(Refusal::NOT_DREAMING),
                    Err(_) => {
                        call.core.sessions.forget(call.session).await;
                        return Err(Refusal::STOPPED);
                    }
                },
                _ => return Err(Refusal::BAD_PARAMS),
            };
            Ok(Ran {
                events: Vec::new(),
                said,
            })
        })
    }
}

/// `/clear-memory`（别名 `/reset-memory`）：只删、不请求模型，照顺序答（项目主人 2026-10-15 定名字、不先问）。
struct ClearMemory;

impl Slash for ClearMemory {
    fn name(&self) -> &'static str {
        "clear-memory"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["reset-memory"]
    }

    fn package(&self) -> &'static str {
        PACKAGE
    }

    fn summary(&self) -> &'static str {
        "commands/summary/clear-memory"
    }

    fn allowed(&self, core: &Core, handle: &Handle, _by: &By) -> Result<(), Refusal> {
        remembering(core, handle)
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let cleared =
                super::clear_in(call.core, call.handle(), call.id, call.by.clone()).await?;
            Ok(Ran {
                events: Vec::new(),
                said: cleared_said(cleared),
            })
        })
    }
}

/// `/clear-memory` 的回执：清了几条（一条的发 `/one`，英文说单数）；一条都没有的说这个会话没有记下的记忆。
pub(super) fn cleared_said(cleared: usize) -> Said {
    match cleared {
        0 => Said::plain("commands/cleared-memory-none"),
        1 => Said {
            key: "commands/cleared-memory/one",
            fields: vec![("count", "1".to_string())],
        },
        _ => Said {
            key: "commands/cleared-memory",
            fields: vec![("count", cleared.to_string())],
        },
    }
}

/// `/dream` 的回执：几样数；摘要换了的多半句，什么都没交进去的说没有要整理的。叫停了的说中断了、剩下的下次整理（施工
/// R-22），一批都没合完的不报数。
fn dreamed_said(dreamed: &miyu_session::Dreamed) -> Said {
    match (dreamed.stopped, dreamed.given) {
        (true, 0) => return Said::plain("commands/dreamed-stopped-nothing"),
        (false, 0) => return Said::plain("commands/dreamed-nothing"),
        _ => {}
    }
    Said {
        key: match (dreamed.stopped, dreamed.summary) {
            (true, _) => "commands/dreamed-stopped",
            (false, true) => "commands/dreamed-summary",
            (false, false) => "commands/dreamed",
        },
        fields: vec![
            ("given", dreamed.given.to_string()),
            ("revised", dreamed.revised.to_string()),
            ("retired", dreamed.retired.to_string()),
        ],
    }
}
