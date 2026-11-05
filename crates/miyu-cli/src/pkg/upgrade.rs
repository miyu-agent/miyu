//! `miyu pkg upgrade`（施工 G-5，设计 `37-软件源.md` 第四节第 6、7 条，pacman 的 `-Su`、`-Syu`）：先盘点（`package.upgrades`），
//! 软件源里没了的、被拉黑的、来源仓库变了的、`packages.hold` 跳过的在标准错误上各说一句；有要升的照 pacman 印「将升级」几行、
//! 下多大，问一句，再升（`package.upgrade`），升好了一个一行。没有要升的说一句、退出码 0。
//!
//! 施工 PK-3 下（设计 `61-装包说清来源和能力.md` 定了的 5、6）：一律先看一眼（`--yes` 也看：新多要的能力要照它批），每个包下面
//! 列新多要的能力、一起装的新依赖列它要的，社区的接「注意」；答「是」、`--yes` 都带 `approve`，升好以后说批了哪些。

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::{PkgPlan, approve, ask, confirm, lines};
use crate::config::columns;
use crate::diagnostic::{self, Problem};
use crate::language::{Doing, Language};
use crate::rpc::Rpc;
use crate::shown::say;

/// 升级。
pub(super) async fn run(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    let survey = ask(rpc, "package.upgrades", json!({}), language, err).await?;
    for problem in notices(&survey, language) {
        diagnostic::warning(err, *language, &problem);
    }
    let upgrades = survey["upgrades"].as_array().cloned().unwrap_or_default();
    if upgrades.is_empty() {
        say(out, language.nothing_to_upgrade());
        return Ok(());
    }
    // 写了 `--yes` 的也看一眼（施工 PK-3 下）：新多要的能力要照它批（设计 61 定了的 5、6）。
    let preview = ask(
        rpc,
        "package.upgrade",
        json!({"preview": true}),
        language,
        err,
    )
    .await?;
    if !plan.args.yes {
        lines(out, rows(&upgrades, &preview, language));
        confirm::asked(
            (input, out, err),
            &question(&upgrades, &preview, language),
            language,
        )?;
    }
    // 答「是」、`--yes` 都算批：看一眼列出的能力照原样带上（旧的核心看一眼里没有能力，表是空的、不带）。
    let items = approve::upgrading(&preview);
    let params = approve::carried(json!({}), &approve::table(&items));
    let done = ask(rpc, "package.upgrade", params, language, err).await?;
    for row in done["upgraded"].as_array().into_iter().flatten() {
        say(
            out,
            &language.upgraded(
                row["package"].as_str().unwrap_or_default(),
                row["from"].as_str().unwrap_or("?"),
                row["to"].as_str().unwrap_or_default(),
            ),
        );
    }
    lines(out, approve::said(&items, language));
    Ok(())
}

/// 确认屏（施工 G-5 起，PK-3 下补能力）：一个包一行「将升级 <编号> <旧> → <新>」，下面一项一行新多要的能力（批过的不印）；一起装的
/// 新依赖一行「将安装 <编号> <版本>」，下面一项一行它要的能力；最后「下载：」。
fn rows(upgrades: &[Value], preview: &Value, language: &Language) -> Vec<String> {
    let looked = |id: &str| {
        preview["upgrades"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|item| item["package"] == id)
    };
    let mut rows = Vec::new();
    for row in upgrades {
        rows.push(change(row, language));
        if let Some(item) = looked(row["package"].as_str().unwrap_or_default()) {
            rows.extend(needs(item, true, language));
        }
    }
    for item in preview["with"].as_array().into_iter().flatten() {
        let id = item["package"].as_str().unwrap_or_default();
        let version = item["version"].as_str().unwrap_or_default();
        rows.push(
            format!("{} {version}", language.will(Doing::Install, id))
                .trim_end()
                .to_string(),
        );
        rows.extend(needs(item, false, language));
    }
    if let Some(bytes) = preview["download"].as_u64() {
        rows.push(format!(
            "{}{}",
            language.plan_label(crate::language::PlanLabel::Download),
            super::query::size(bytes)
        ));
    }
    rows
}

/// 一个包下面的能力：「  新要：」（升的，`fresh`）或「  需要：」（一起装的）开头，后面几行缩进到同一列。
fn needs(item: &Value, fresh: bool, language: &Language) -> Vec<String> {
    let label = language.upgrade_needs(fresh);
    let indent = " ".repeat(columns(label));
    approve::needs(item, fresh, language)
        .iter()
        .enumerate()
        .map(|(index, line)| match index {
            0 => format!("{label}{line}"),
            _ => format!("{indent}{line}"),
        })
        .collect()
}

/// 问的那一句：一个的「升级 '<编号>'？」，几个的「升级 N 个软件包？」；有新要的能力的换成「并批准新要的能力」（施工 PK-3 下）。
/// 核心给了 `notice` 的前面先一行「注意：…」。
fn question(upgrades: &[Value], preview: &Value, language: &Language) -> String {
    let approving = approve::asks(&approve::upgrading(preview));
    let asked = match (upgrades, approving) {
        ([one], false) => {
            language.proceed(Doing::Upgrade, one["package"].as_str().unwrap_or_default())
        }
        ([one], true) => {
            language.proceed_approving(Doing::Upgrade, one["package"].as_str().unwrap_or_default())
        }
        (many, false) => language.proceed_upgrades(many.len()),
        (many, true) => language.proceed_many_approving(Doing::Upgrade, many.len()),
    };
    approve::noticed(approve::notice(&preview["notice"], language), asked)
}

/// 「将升级 app 1.0 → 2.0」。
fn change(row: &Value, language: &Language) -> String {
    format!(
        "{} {} → {}",
        language.will(Doing::Upgrade, row["package"].as_str().unwrap_or_default()),
        row["from"].as_str().unwrap_or("?"),
        row["to"].as_str().unwrap_or_default()
    )
}

/// 盘点里要说一句的：一条一个警告。
fn notices(survey: &Value, language: &Language) -> Vec<Problem> {
    let text = |row: &Value, key: &str| row[key].as_str().unwrap_or_default().to_string();
    let items = |key: &str| survey[key].as_array().cloned().unwrap_or_default();
    let mut said = Vec::new();
    for id in items("gone") {
        said.push(language.upgrade_gone(id.as_str().unwrap_or_default()));
    }
    for row in items("blocked") {
        said.push(language.upgrade_blocked(&text(&row, "package"), &text(&row, "reason")));
    }
    for row in items("moved") {
        said.push(language.upgrade_moved(&text(&row, "package"), &text(&row, "now")));
    }
    for id in items("held") {
        said.push(language.upgrade_held(id.as_str().unwrap_or_default()));
    }
    said
}

#[cfg(test)]
mod tests;
