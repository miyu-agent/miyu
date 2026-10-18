//! 装（施工 F-8 下补起，G-4 下从 `pkg.rs` 挪出来：`install` 和 pacman 的 `-S <编号>` 共用）：先看一眼，没写 `--yes` 的照 pacman
//! 印要做什么、问一句；装好了问要不要打开。从软件源装的看一眼另有一起装的依赖、要下多大、来源（`confirm.rs`），装之前说一句
//! 正在下载。

use std::io::{BufRead, Write};

use serde_json::Value;

use super::{PkgPlan, ask, confirm, lines, open, package_of, previewed};
use crate::language::Doing;
use crate::rpc::Rpc;
use crate::shown::say;

/// 照 `params`（`path`、`package` 或 `source`）装一个。
pub(super) async fn run(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    params: Value,
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    // 看一眼：没写 `--yes` 的印出来、问一句；要的能力装好以后打开时一并批（作者说明第九节第 1 条）。
    let shown = ask(rpc, "package.install", previewed(&params), language, err).await?;
    let download = shown["download"].as_u64();
    if !plan.args.yes {
        let doing = match (params.get("path"), download) {
            (Some(_), _) | (None, Some(_)) => Doing::Install,
            (None, None) => Doing::Restore,
        };
        lines(out, confirm::plan(&shown, doing, language));
        confirm::asked(input, out, err, doing, language)?;
    }
    if let Some(bytes) = download {
        say(out, &language.downloading(&super::query::size(bytes)));
    }
    let result = ask(rpc, "package.install", params, language, err).await?;
    let id = package_of(&result);
    for with in result["with"].as_array().into_iter().flatten() {
        say(out, &language.installed(with.as_str().unwrap_or_default()));
    }
    say(out, &language.installed(id));
    if !open::wanted(&result, &shown) {
        return Ok(());
    }
    if !open::agreed(input, out, plan.args.yes, language) {
        say(out, language.not_opened());
        return Ok(());
    }
    open::open(rpc, id, &open::capabilities(&shown), language, err).await?;
    say(out, &language.opened(id));
    Ok(())
}
