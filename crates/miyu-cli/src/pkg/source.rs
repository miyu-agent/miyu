//! `miyu pkg` 里软件源的几个（施工 G-3 上起，设计 `37-软件源.md` 第四节）：`update`（pacman 的 `-Sy`）拿新的索引。之后的
//! `search`、从软件源装、`upgrade` 也住这里（`pkg.rs` 到了 500 行）。

use std::io::Write;

use serde_json::json;

use super::ask;
use crate::exit;
use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::say;

/// `update`：不管本机那份新不新都去拿（`package.sync` 带 `force`）。拿成了印包数和索引的时刻；一处都没拿成、用的是本机那份的
/// 说一句、退出码 1（照 pacman：`-Sy` 没拿成算失败）。`json` 的原样印回应。
pub(super) async fn update(
    rpc: &mut Rpc,
    language: &Language,
    json: bool,
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let synced = ask(rpc, "package.sync", json!({"force": true}), language, err).await?;
    if json {
        say(out, &synced.to_string());
    }
    let generated = synced["generated"].as_str().unwrap_or_default();
    if let Some(why) = synced["stale"].as_str() {
        say(err, &language.index_stale(generated, why));
        return Err(exit::ERROR);
    }
    if !json {
        let count = synced["packages"].as_u64().unwrap_or_default();
        say(out, &language.index_updated(count, generated));
    }
    Ok(())
}
