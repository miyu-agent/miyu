//! 一条命令的几种写法（施工 O-6 四补的 [`Slash::usage`]）照连接的语言写好：`command.catalog` 的 `usage` 照它列，参数写错的
//! 拒绝照它带上用法（施工 LO-1，`protocol.md`「`command.run`」）。两处同一份字表，不另写一套。

use super::{Said, Slash, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `slash` 的几种写法，照先后：`/<名字>` 接上后面跟的（`commands/` 开头的照连接的语言换成那一句，施工 O-6 五补）、做什么那一句。
/// 只有一种写法的是空的。
pub(super) async fn forms(core: &Core, peer: &Peer, slash: &dyn Slash) -> Vec<(String, String)> {
    let mut usage = Vec::new();
    for (rest, said) in slash.usage() {
        let rest = match rest.starts_with("commands/") {
            true => words(core, peer, &Said::plain(rest)).await,
            false => (*rest).to_string(),
        };
        let form = match rest.as_str() {
            "" => format!("/{}", slash.name()),
            rest => format!("/{} {rest}", slash.name()),
        };
        usage.push((form, words(core, peer, &Said::plain(said)).await));
    }
    usage
}

/// 参数写错了：`bad_params`，`data.why` 照连接的语言写「参数不对，用法：」，下面一行一种写法（`commands/usage-line`）。
pub(super) async fn refused(core: &Core, peer: &Peer, slash: &dyn Slash) -> Refusal {
    let mut lines = vec![words(core, peer, &Said::plain("commands/bad-usage")).await];
    for (form, summary) in forms(core, peer, slash).await {
        let line = Said::with(
            "commands/usage-line",
            vec![("form", form), ("summary", summary)],
        );
        lines.push(words(core, peer, &line).await);
    }
    Refusal::BAD_PARAMS.because(lines.join("\n"))
}
