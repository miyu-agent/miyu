//! 装（施工 F-8 下补起，G-4 下从 `pkg.rs` 挪出来：`install` 和 pacman 的 `-S <编号>` 共用）：先看一眼，没写 `--yes` 的照 pacman
//! 印要做什么、问一句；装好了问要不要打开。从软件源装的看一眼另有一起装的依赖、要下多大、来源（`confirm.rs`），装之前说一句
//! 正在下载。

use std::io::{BufRead, Write};

use serde_json::Value;

use super::{PkgPlan, ask_about, confirm, lines, open, package_of, previewed};
use crate::language::Doing;
use crate::rpc::Rpc;
use crate::shown::say;

/// 照 `params`（`path`、`package` 或 `source`）装一个；`what` 是人写的那一个（报错里照它说）。
pub(super) async fn run(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (params, what): (Value, &str),
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    // 看一眼：没写 `--yes` 的印出来、问一句；要的能力装好以后打开时一并批（作者说明第九节第 1 条）。
    let preview = ("package.install", previewed(&params), Some(what));
    let shown = ask_about(rpc, preview, language, err).await?;
    // 写的是组名的（施工 BS-1 下中）：组里卸掉了的一起装回来。
    if super::group::is_group(&shown) {
        return super::group::run(
            rpc,
            plan,
            ("package.install", params, &shown),
            (input, out, err),
        )
        .await;
    }
    let download = shown["download"].as_u64();
    if !plan.args.yes {
        let doing = match (params.get("path"), download) {
            (Some(_), _) | (None, Some(_)) => Doing::Install,
            (None, None) => Doing::Restore,
        };
        lines(out, confirm::plan(&shown, doing, language));
        let id = shown["package"].as_str().unwrap_or(what);
        confirm::asked((input, out, err), &language.proceed(doing, id), language)?;
    }
    if let Some(bytes) = download {
        say(out, &language.downloading(&super::query::size(bytes)));
    }
    let result = ask_about(rpc, ("package.install", params, Some(what)), language, err).await?;
    let id = package_of(&result);
    for with in result["with"].as_array().into_iter().flatten() {
        say(out, &language.installed(with.as_str().unwrap_or_default()));
    }
    let version = result["version"]
        .as_str()
        .filter(|version| !version.is_empty());
    let installed = version.map_or_else(|| id.to_string(), |version| format!("{id} {version}"));
    say(out, &language.installed(&installed));
    if !open::wanted(&result, &shown) {
        return Ok(());
    }
    if !open::agreed(input, out, (plan.args.yes, id), language) {
        lines(out, language.not_opened(id).to_vec());
        return Ok(());
    }
    open::open(rpc, id, &open::capabilities(&shown), language, err).await?;
    say(out, &language.opened(id));
    Ok(())
}
