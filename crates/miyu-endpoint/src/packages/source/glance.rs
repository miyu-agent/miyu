//! 从软件源装之前看一眼（施工 G-4 上起，IN-3 从 `fetch.rs` 挪出来、多推荐的和装好以后多大，设计 `49-装包照pacman.md` 第三节
//! 第 3 条）：只照索引说，不下载。施工 PK-3 上起每个包说清从哪来（`source`：`official`、`community`）、哪个软件源（`feed`）、
//! 要的能力（`capabilities`，这一刻批过、装完照旧留着的带 `held`），社区的另有「注意」（设计 `61-装包说清来源和能力.md` 第二节）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Map, Value, json};

use miyu_source::index::Entry;
use miyu_store::packages::Found;

use super::super::notice::{self, Item};
use super::super::{approve, packages};
use super::feeds::{Feed, Tier};
use super::fetch::{Indexes, Step, came, pick};
use crate::Core;

/// 看一眼要用的：核心、连接的语言、开关文件里批过的、本地库在哪（照它判断来路变没变）。
pub(super) struct Looking<'a> {
    core: &'a Core,
    language: &'a str,
    approved: BTreeMap<String, Vec<String>>,
    local: PathBuf,
}

impl<'a> Looking<'a> {
    /// 读一次开关文件。
    pub(super) fn new(core: &'a Core, language: &'a str) -> Looking<'a> {
        Looking {
            core,
            language,
            approved: crate::extensions::read_switches(core).0.approved,
            local: packages(core).local_root(),
        }
    }

    /// 要的能力：索引条目写的，照连接的语言说；这一刻批过、从这个源照这个来源仓库装完照旧留着的标 `held`。
    fn capabilities(&self, feed: &Feed, entry: &Entry) -> Value {
        let mut listed = crate::extensions::named(
            self.core,
            entry.capabilities.iter().map(String::as_str),
            self.language,
        );
        let kept = approve::kept(
            &self.approved,
            &self.local,
            &entry.id,
            Some(&came(feed, entry)),
        );
        approve::mark_held(&mut listed, &kept);
        listed
    }

    /// 一个包的那几格：编号、版本、`source`、`feed`、`capabilities`（声明了的才有）。
    pub(super) fn item(&self, feed: &Feed, entry: &Entry) -> Map<String, Value> {
        let mut one = Map::new();
        one.insert("package".into(), json!(entry.id));
        one.insert("version".into(), json!(entry.version));
        one.insert("source".into(), json!(source(feed, entry)));
        one.insert("feed".into(), json!(feed.name));
        if !entry.capabilities.is_empty() {
            one.insert("capabilities".into(), self.capabilities(feed, entry));
        }
        one
    }

    /// 几个包要说的「注意」：社区的写出来，带会跑的东西的再说一句；`alone` 的是这一次只有一个包。
    pub(super) fn notice(&self, items: &[(&Feed, &Entry)], alone: bool) -> Option<Value> {
        let items: Vec<Item<'_>> = items
            .iter()
            .map(|(feed, entry)| Item {
                id: &entry.id,
                source: source(feed, entry),
                runs: notice::entry_runs(entry),
            })
            .collect();
        notice::notice(self.core, self.language, &items, alone)
    }
}

/// 从哪来：官方源里官方的条目是 `official`；官方源里社区收录的、配置里自己加的源里的一律是 `community`（加源是信那把钥匙，
/// 包里的东西还是没人审过）。
pub(super) fn source(feed: &Feed, entry: &Entry) -> &'static str {
    match entry.official && feed.tier == Tier::Official {
        true => "official",
        false => "community",
    }
}

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
    let looking = Looking::new(core, language);
    // 和从包文件夹装的看一眼同一个形状（命令行、头照同一套印），带了哪几样照索引摊开；另有下多大、一起装的、来源。
    let mut answer = Value::Object(looking.item(last.feed, entry));
    // 要装的那一个的能力照旧总有这一格（没声明的是空的）：没跟上的头照它拼。
    answer["capabilities"] = looking.capabilities(last.feed, entry);
    // 别的源里的不算官方（施工 U23 下下）：那一格是它的索引自己写的。
    answer["official"] = json!(source(last.feed, entry) == "official");
    answer["origin"] = json!(entry.origin);
    answer["download"] = json!(steps.iter().map(|step| step.file.size).sum::<u64>());
    answer["with"] = json!(
        before
            .iter()
            .map(|step| looking.item(step.feed, step.entry))
            .collect::<Vec<_>>()
    );
    let recommended = recommends(indexes, steps, now);
    answer["recommends"] = json!(
        recommended
            .iter()
            .filter_map(|(feed, entry)| recommended_item(&looking, feed, entry))
            .collect::<Vec<_>>()
    );
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
    // 「注意」（施工 PK-3 上）：这一次装的几个一起说；推荐的另问，另说一份。
    let installing: Vec<(&Feed, &Entry)> =
        steps.iter().map(|step| (step.feed, step.entry)).collect();
    if let Some(said) = looking.notice(&installing, steps.len() == 1) {
        answer["notice"] = said;
    }
    let offered: Vec<(&Feed, &Entry)> = recommended
        .iter()
        .filter(|(_, entry)| pick(entry).is_some())
        .map(|(feed, entry)| (*feed, *entry))
        .collect();
    if let Some(said) = looking.notice(&offered, false) {
        answer["recommends_notice"] = said;
    }
    answer
}

/// 要装的几个推荐的、还没装的（装着的版本不够下限的也算，施工 PK-2）、这一次也不装的，去重，照要装的先后、各自写的先后：在哪个
/// 源、索引里那一条。索引里找不到的、版本不够下限的不列。
fn recommends<'a>(
    indexes: &'a Indexes,
    steps: &[Step<'_>],
    now: &[Found],
) -> Vec<(&'a Feed, &'a Entry)> {
    let mut skip: BTreeSet<String> = steps.iter().map(|step| step.entry.id.clone()).collect();
    let mut listed = Vec::new();
    for dep in steps.iter().flat_map(|step| step.entry.recommended()) {
        let enough = now.iter().any(|one| {
            let version = one
                .read
                .as_ref()
                .ok()
                .and_then(|read| read.version.as_deref());
            one.id == dep.id && dep.fits(version)
        });
        if enough || !skip.insert(dep.id.clone()) {
            continue;
        }
        let found = indexes.iter().find_map(|(feed, index)| {
            let entry = index.entry(&dep.id)?;
            dep.fits(Some(&entry.version)).then_some((feed, entry))
        });
        if let Some(found) = found {
            listed.push(found);
        }
    }
    listed
}

/// 推荐的一个：同一起装的那几格，另有本机那份文件多大、装好以后多大（索引写了的）。没有本机能用的文件的没有。
fn recommended_item(looking: &Looking<'_>, feed: &Feed, entry: &Entry) -> Option<Value> {
    let file = pick(entry)?;
    let mut one = looking.item(feed, entry);
    one.insert("download".into(), json!(file.size));
    if let Some(bytes) = entry.installed {
        one.insert("installed_size".into(), json!(bytes));
    }
    Some(Value::Object(one))
}
