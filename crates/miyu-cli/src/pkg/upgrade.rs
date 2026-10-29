//! `miyu pkg upgrade`（施工 G-5，设计 `37-软件源.md` 第四节第 6、7 条，pacman 的 `-Su`、`-Syu`）：先盘点（`package.upgrades`），
//! 软件源里没了的、被拉黑的、来源仓库变了的、`packages.hold` 跳过的在标准错误上各说一句；有要升的照 pacman 印「将升级」几行、
//! 下多大，问一句，再升（`package.upgrade`），升好了一个一行。没有要升的说一句、退出码 0。

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::{PkgPlan, ask, confirm, lines};
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
    if !plan.args.yes {
        let preview = ask(
            rpc,
            "package.upgrade",
            json!({"preview": true}),
            language,
            err,
        )
        .await?;
        let mut rows: Vec<String> = upgrades.iter().map(|row| change(row, language)).collect();
        if let Some(bytes) = preview["download"].as_u64() {
            rows.push(format!(
                "{}{}",
                language.plan_label(crate::language::PlanLabel::Download),
                super::query::size(bytes)
            ));
        }
        lines(out, rows);
        let question = match upgrades.as_slice() {
            [one] => language.proceed(Doing::Upgrade, one["package"].as_str().unwrap_or_default()),
            many => language.proceed_upgrades(many.len()),
        };
        confirm::asked((input, out, err), &question, language)?;
    }
    let done = ask(rpc, "package.upgrade", json!({}), language, err).await?;
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
    Ok(())
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
