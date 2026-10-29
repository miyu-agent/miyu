//! 从软件源装之前看一眼（施工 G-4 上起，IN-3 从 `fetch.rs` 挪出来、多推荐的和装好以后多大，设计 `49-装包照pacman.md` 第三节
//! 第 3 条）：只照索引说，不下载。

use std::collections::BTreeSet;

use serde_json::{Value, json};

use miyu_source::index::Entry;
use miyu_store::packages::Found;

use super::feeds::Tier;
use super::fetch::{Indexes, Step, pick};
use crate::Core;

/// 看一眼：要装的那个、一起装的依赖、要下多大、装好以后一共多大（`installed`，有一个索引没写的不写这一格）、推荐的还没装的
/// （`recommends`）；换下哪一份照装进的那一层看得到的 `now` 认。要的能力同从包文件夹装的那一份（编号、照连接的语言 `language`
/// 说的名字，施工 IN-3）。
pub(super) fn preview(
    (core, language): (&Core, &str),
    indexes: &Indexes,
    steps: &[Step<'_>],
    now: &[Found],
) -> Value {
    let Some((last, before)) = steps.split_last() else {
        return Value::Null;
    };
    let entry = last.entry;
    // 和从包文件夹装的看一眼同一个形状（命令行、头照同一套印），带了哪几样照索引摊开；另有下多大、一起装的、来源。
    let mut answer = json!({
        "package": entry.id,
        "version": entry.version,
        // 别的源里的不算官方（施工 U23 下下）：那一格是它的索引自己写的。
        "official": entry.official && last.feed.tier == Tier::Official,
        "source": last.feed.name,
        "origin": entry.origin,
        "capabilities": crate::extensions::named(core, entry.capabilities.iter().map(String::as_str), language),
        "download": steps.iter().map(|step| step.file.size).sum::<u64>(),
        "with": before.iter().map(|step| json!({"package": step.entry.id, "version": step.entry.version})).collect::<Vec<_>>(),
        "recommends": recommends(indexes, steps, now),
    });
    let installed: Option<u64> = steps.iter().map(|step| step.entry.installed).sum();
    if let Some(bytes) = installed {
        answer["installed"] = json!(bytes);
    }
    for (kind, names) in &entry.provides {
        match kind.as_str() {
            "command" => answer["command"] = json!(names.first()),
            "features" => {}
            _ => answer[kind.as_str()] = json!(names),
        }
    }
    if let Some(old) = now.iter().find(|one| one.id == entry.id) {
        let version = old.read.as_ref().ok().and_then(|old| old.version.clone());
        answer["replaces"] = json!({ "version": version });
    }
    answer
}

/// 要装的几个推荐的、还没装的、这一次也不装的，去重，照要装的先后、各自写的先后：编号、版本、本机那份文件多大、装好以后多大
/// （索引写了的）。索引里找不到的、没有本机能用的文件的不列。
fn recommends(indexes: &Indexes, steps: &[Step<'_>], now: &[Found]) -> Vec<Value> {
    let mut skip: BTreeSet<&str> = now.iter().map(|one| one.id.as_str()).collect();
    skip.extend(steps.iter().map(|step| step.entry.id.as_str()));
    let mut listed = Vec::new();
    for id in steps.iter().flat_map(|step| &step.entry.recommends) {
        if !skip.insert(id.as_str()) {
            continue;
        }
        let Some(entry) = indexes.iter().find_map(|(_, index)| index.entry(id)) else {
            continue;
        };
        if let Some(one) = recommended(entry) {
            listed.push(one);
        }
    }
    listed
}

/// 推荐的一个：没有本机能用的文件的没有。
fn recommended(entry: &Entry) -> Option<Value> {
    let file = pick(entry)?;
    let mut one = json!({"package": entry.id, "version": entry.version, "download": file.size});
    if let Some(bytes) = entry.installed {
        one["installed_size"] = json!(bytes);
    }
    Some(one)
}
