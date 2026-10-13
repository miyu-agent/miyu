//! `miyu kb` 列、搜的回应怎么印（施工 K-8 从 `kb.rs` 挪出来）：`json` 的原样印，`text` 的照 `shown.rs` 的样子；列表里有软件包
//! 带来的库的，先问一次包的名字。

use std::collections::BTreeMap;
use std::io::Write;

use serde_json::{Value, json};

use super::{KbPlan, ask, shown};
use crate::ask::Format;
use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::say;

/// `list` 的回应印出来：`json` 的原样印那一串；`text` 的一行一个库，一个都没有的说一句。
/// 列表里有软件包带来的库的，问一次 `package.list` 拿包的名字（施工 K-8，照界面语言）；没有的不问。
pub(super) async fn package_names(
    rpc: &mut Rpc,
    listed: &Value,
    language: &Language,
    err: &mut dyn Write,
) -> Result<BTreeMap<String, String>, u8> {
    let bases = listed["bases"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !bases.iter().any(|base| base["package"].is_string()) {
        return Ok(BTreeMap::new());
    }
    let result = ask(rpc, "package.list", json!({}), language, err).await?;
    Ok(result["packages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|one| {
            Some((
                one["package"].as_str()?.to_string(),
                one["name"].as_str()?.to_string(),
            ))
        })
        .collect())
}

pub(super) fn list(
    result: &Value,
    names: &BTreeMap<String, String>,
    plan: &KbPlan,
    out: &mut dyn Write,
) -> Result<(), u8> {
    let bases = result["bases"].as_array().cloned().unwrap_or_default();
    if plan.args.format == Format::Json {
        say(out, &Value::Array(bases).to_string());
        return Ok(());
    }
    if bases.is_empty() {
        say(out, plan.language.no_bases());
        return Ok(());
    }
    for row in shown::bases(&bases, names, &plan.language) {
        say(out, &row);
    }
    Ok(())
}

/// `search` 的回应印出来：`json` 的原样印那一串；`text` 的一块一段，段和段之间空一行，一块都没有的说一句。
pub(super) fn search(result: &Value, plan: &KbPlan, out: &mut dyn Write) -> Result<(), u8> {
    let hits = result["hits"].as_array().cloned().unwrap_or_default();
    if plan.args.format == Format::Json {
        say(out, &Value::Array(hits).to_string());
        return Ok(());
    }
    if hits.is_empty() {
        say(out, plan.language.nothing_found());
        return Ok(());
    }
    say(out, &shown::hits(&hits, &plan.language));
    Ok(())
}
