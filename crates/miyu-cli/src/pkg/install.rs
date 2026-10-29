//! 装（施工 F-8 下补起，G-4 下从 `pkg.rs` 挪出来：`install` 和 pacman 的 `-S <编号>` 共用）：先看一眼，没写 `--yes` 的照 pacman
//! 印要做什么、问一句；装好了问要不要打开。
//!
//! 从软件源装的照 pacman 分阶段印（施工 IN-3，设计 `49-装包照pacman.md` 第二节）：看一眼之前在标准错误上说正在解析依赖、检查
//! 冲突；印要装的、推荐的和几行「键 : 值」（`summary.rs`），问装不装，有推荐的再问一句；装的时候照核心推的进度印下载、核对、安装
//! （`progress.rs`）；推荐的答了「是」的，主包装完以后一个一个照同一条请求装，不再问、不再列它们推荐的。`--yes` 主包当「是」、
//! 推荐的当「否」。从包目录、包文件装的、装回出厂的照旧印那一份。

use std::collections::BTreeSet;
use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::progress::Progress;
use super::{PkgPlan, ask_about, confirm, lines, open, package_of, previewed, refused, summary};
use crate::diagnostic;
use crate::exit;
use crate::language::{Doing, Language};
use crate::link;
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
    // 写的是编号的：看一眼要先拿索引、排依赖、查冲突，照 pacman 先说一声。
    if params.get("path").is_none() {
        say(err, language.resolving());
        say(err, language.looking_for_conflicts());
    }
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
    let mut recommended = Vec::new();
    match (plan.args.yes, summary::from_source(&shown)) {
        (true, _) => {}
        (false, true) => {
            lines(out, summary::plan(&shown, language));
            let question = summary::question(&shown, language);
            confirm::asked((input, out, err), &question, language)?;
            let wanted = summary::recommends(&shown);
            let also = language.proceed_recommended(wanted.len());
            if !wanted.is_empty() && confirm::agreed((input, out), &also) {
                recommended = wanted;
            }
        }
        (false, false) => {
            let doing = match params.get("path") {
                Some(_) => Doing::Install,
                None => Doing::Restore,
            };
            lines(out, confirm::plan(&shown, doing, language));
            let id = shown["package"].as_str().unwrap_or(what);
            confirm::asked((input, out, err), &language.proceed(doing, id), language)?;
        }
    }
    let mut done = one(rpc, plan, (params, what, &shown), (input, out, err)).await?;
    for (id, _) in recommended {
        if !done.insert(id.clone()) {
            continue;
        }
        let params = json!({ "source": id });
        let preview = ("package.install", previewed(&params), Some(id.as_str()));
        let shown = ask_about(rpc, preview, language, err).await?;
        done.extend(one(rpc, plan, (params, &id, &shown), (input, out, err)).await?);
    }
    Ok(())
}

/// 真装一次，照看一眼的 `shown` 印进度；装好了印一行、问要不要打开。交回这一次装上的编号（连一起装的依赖）。
async fn one(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (params, what, shown): (Value, &str, &Value),
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<BTreeSet<String>, u8> {
    let language = &plan.language;
    let mut progress = Progress::new(language, plan.terminal, &summary::packages(shown));
    let request = ("package.install", params, Some(what));
    let result = watching(rpc, request, language, (err, &mut progress)).await?;
    let id = package_of(&result);
    let mut done = BTreeSet::from([id.to_string()]);
    for with in result["with"].as_array().into_iter().flatten() {
        let with = with.as_str().unwrap_or_default();
        done.insert(with.to_string());
        say(out, &language.installed(with));
    }
    let version = result["version"]
        .as_str()
        .filter(|version| !version.is_empty());
    let installed = version.map_or_else(|| id.to_string(), |version| format!("{id} {version}"));
    say(out, &language.installed(&installed));
    if !open::wanted(&result, shown) {
        return Ok(done);
    }
    if !open::agreed(input, out, (plan.args.yes, id), language) {
        lines(out, language.not_opened(id).to_vec());
        return Ok(done);
    }
    open::open(rpc, id, &open::capabilities(shown), language, err).await?;
    say(out, &language.opened(id));
    Ok(done)
}

/// 同 [`super::ask_about`]，等回应时核心推来的进度交给 `progress` 印在标准错误上；印到一半的那一行在说结果、报错之前换行。
async fn watching(
    rpc: &mut Rpc,
    (method, params, what): (&str, Value, Option<&str>),
    language: &Language,
    (err, progress): (&mut dyn Write, &mut Progress<'_>),
) -> Result<Value, u8> {
    let answered =
        link::answer_watching(rpc, (method, params), language, err, &mut |message, err| {
            progress.on(message, err)
        })
        .await;
    progress.finish(err);
    match answered? {
        Ok(result) => Ok(result),
        Err(error) => {
            for problem in refused(&error, what, language) {
                diagnostic::error(err, *language, &problem);
            }
            Err(exit::ERROR)
        }
    }
}
