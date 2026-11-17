//! `/life` 在不带人格的会话里（施工 LF-12，`docs/blueprint/life.md`「命令」第 8 条；项目主人 2026-11-17 本机验收第 25 条）：
//! 人生是照人格算的，这个会话没有人格就照所有人格的找（[`crate::life::lives`]：没结束的，一个人格至多一条）。
//!
//! - 只有一个人格有：照它办，同带着这个人格的会话里打的一样，只是回执写上是哪个人格的（名字，没起名字的写编号）。
//! - 不止一个：不猜。列出来（照人格的编号排，在跑的、停着的各写各的），不带要跳过去的会话，什么都不动。
//! - 一个都没有：光打的回说明，写明这个会话没有人格、先选一个；停、续、结束回没有。
//! - `/life new` 不看有几个：不猜给谁开，回一句先选人格。
//! - 光打的照样带 `lives`（同带人格的会话里），头照它让人挑着跳过去。

use serde_json::Value;

use super::{Asked, listing, ran};
use crate::commands::Said;
use crate::commands::registry::{Call, Ran};
use crate::life::Heaped;
use crate::refusal::Refusal;

/// 在不带人格的会话里做 `asked`。
pub(super) fn run<'a>(call: &'a Call<'a>, asked: Asked) -> Heaped<'a, Result<Ran, Refusal>> {
    Box::pin(async move {
        if matches!(asked, Asked::New) {
            return Ok(ran(Said::plain("commands/life-new-bare"), None));
        }
        let lives = crate::life::lives(call.core, call.peer.language).await?;
        let mut done = match lives.as_slice() {
            [] => {
                let key = match asked {
                    Asked::Show => "commands/life-about-bare",
                    _ => "commands/life-nothing-bare",
                };
                ran(Said::plain(key), None)
            }
            [only] => {
                let persona = only["persona"].as_str().unwrap_or_default();
                let name = only["name"].as_str().unwrap_or(persona);
                super::run(call.core, call.id, persona, asked, Some(name)).await?
            }
            several => ran(listed(several), None),
        };
        if matches!(asked, Asked::Show) {
            listing(&mut done, lives, None);
        }
        Ok(done)
    })
}

/// 不止一个人格有人生会话的那一句：一个一段（名字，在跑还是停着），照这门语言连起来。
fn listed(lives: &[Value]) -> Said {
    let parts = lives
        .iter()
        .map(|one| {
            let key = match one["state"].as_str() {
                Some("running") => "commands/life-one-running",
                _ => "commands/life-one-paused",
            };
            let name = one["name"].as_str().unwrap_or_default().to_string();
            Said::with(key, vec![("persona", name)])
        })
        .collect();
    Said {
        key: "commands/life-several",
        fields: Vec::new(),
        lists: vec![("lives", parts)],
    }
}
