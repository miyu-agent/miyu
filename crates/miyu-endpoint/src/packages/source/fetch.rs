//! 从软件源装（施工 G-4 上，设计 `37-软件源.md` 第四节第 4 条）：`package.install {"source": "<编号>", "preview"?}`。
//!
//! - 先照 [`super::sync`] 拿索引；编号不在索引里的 `unknown_package`。
//! - 依赖照 pacman 自动补：缺的依赖从索引里一起装，先装依赖；索引里也没有的 `depends_missing`（`data.missing`）。
//! - 每个包挑本机能用的那个文件（不分系统的、或者写了本机这一种的），没有的 `wrong_target`。
//! - 看一眼只照索引说要装哪几个、要下多大，不下载。
//! - 真装：一次只装一件（`core.packaging`）；一个一个下（[`super::download`]，进度推 `package.progress` 给发起的连接），再照
//!   装包文件那一条装（[`super::super::archive`]），本地库记上来源仓库。下不成的 `download_failed`（`data.why` 是每一处的原因），前面装好的留着。

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Target;
use miyu_source::archive;
use miyu_source::index::{Entry, File, Index};
use miyu_store::packages::Packages;

use super::super::{TARGET, packages};
use super::download::{self, Wanted};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::reverse::Peer as Reverse;
use crate::settings::SourceSettings;

/// 进度最多多久推一次。
const EVERY: Duration = Duration::from_millis(200);

/// `package.install` 从软件源装的参数：`source` 写编号；只写 `package` 的、那个编号又不是卸掉的出厂包的也走这里（施工 G-4 下：
/// 装只有一个动词，核心判断是装回出厂的还是从软件源装）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceParams {
    /// 索引里的编号。
    #[serde(alias = "package")]
    source: String,
    /// 只看一眼。
    #[serde(default)]
    preview: bool,
}

impl SourceParams {
    /// 真装编号是 `id` 的（升级照它一个一个换，施工 G-5）。
    pub(crate) fn of(id: &str) -> SourceParams {
        SourceParams {
            source: id.to_string(),
            preview: false,
        }
    }

    /// 看一眼编号是 `id` 的（她的工具问人之前，施工 G-6）。
    pub(crate) fn looking(id: &str) -> SourceParams {
        SourceParams {
            source: id.to_string(),
            preview: true,
        }
    }
}

/// 这一条 `package.install` 是不是从软件源装：写了 `source` 的；只写了 `package`、那个编号不是出厂包的（出厂包的编号照旧是
/// 装回来：卸掉了的装回来，没卸的 `unknown_package`；软件源里也不会有同编号的，装上会撞 `package_exists`）。
pub(crate) fn wanted(core: &Core, params: &Value) -> bool {
    if params.get("source").is_some() {
        return true;
    }
    match (
        params.get("package").and_then(Value::as_str),
        params.get("path"),
    ) {
        (Some(id), None) => !Packages::shipped(&core.resources)
            .read()
            .iter()
            .any(|shipped| shipped.id == id),
        _ => false,
    }
}

/// 要装的一个：索引里那一条、挑好的文件、文件名。
struct Step<'a> {
    entry: &'a Entry,
    file: &'a File,
    name: String,
}

/// 从软件源装 `params.source`。
pub(crate) async fn install(
    core: &Arc<Core>,
    peer: Peer,
    caller: Option<&Reverse>,
    params: SourceParams,
) -> Result<Value, Refusal> {
    if !miyu_store::personas::valid(&params.source) {
        return Err(Refusal::BAD_PARAMS);
    }
    let synced = super::sync::sync(core, false).await?;
    let installed: BTreeSet<String> = core
        .packages()
        .iter()
        .map(|found| found.id.clone())
        .collect();
    let steps = plan(&synced.index, &params.source, &installed)?;
    if params.preview {
        return Ok(preview(&steps, core));
    }
    let _one_at_a_time = core.packaging.lock().await;
    let settings = SourceSettings::from(&core.config().resolved().values());
    let mirrors = super::sync::resolved(settings.mirrors.iter());
    let places = packages(core);
    let cache = download::cache_dir(places.state_root());
    let mut answer = Value::Null;
    let mut with = Vec::new();
    for step in &steps {
        let report = progress(caller, &step.entry.id);
        let wanted = Wanted {
            name: &step.name,
            file: step.file,
            mirrors: &mirrors,
        };
        let file = download::fetch(&cache, &wanted, &report)
            .await
            .map_err(Refusal::download_failed)?;
        answer = super::super::archive::install(core, peer, &file, false).await?;
        let (record, id, origin) = (
            places.clone(),
            step.entry.id.clone(),
            step.entry.origin.clone(),
        );
        let recorded = tokio::task::spawn_blocking(move || {
            super::super::local::set_origin(&record, &id, &origin)
        })
        .await
        .map_err(|_| Refusal::INTERNAL)?;
        if let Err(error) = recorded {
            tracing::warn!(target: TARGET, package = step.entry.id.as_str(), error = %error, "package origin not recorded");
        }
        if step.entry.id != params.source {
            with.push(step.entry.id.clone());
        }
    }
    if !with.is_empty() {
        answer["with"] = json!(with);
    }
    Ok(answer)
}

/// 照依赖排好先后：依赖在前，要装的那个最后；装着的依赖不再装。
fn plan<'a>(
    index: &'a Index,
    id: &str,
    installed: &BTreeSet<String>,
) -> Result<Vec<Step<'a>>, Refusal> {
    let target = index.entry(id).ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let mut order: Vec<&Entry> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    visit(
        index,
        target,
        installed,
        &mut seen,
        &mut order,
        &mut missing,
    );
    if !missing.is_empty() {
        let missing: Vec<&str> = missing.iter().map(String::as_str).collect();
        return Err(Refusal::depends_missing(&missing));
    }
    order
        .into_iter()
        .map(|entry| {
            let file = pick(entry).ok_or(Refusal::WRONG_TARGET)?;
            let target = file.target.as_deref().and_then(Target::parse);
            Ok(Step {
                entry,
                file,
                name: archive::file_name(&entry.id, &entry.version, target),
            })
        })
        .collect()
}

/// 深度优先：先排它的依赖，再排它自己；排过的、装着的依赖跳过（依赖成环的也停得下来）。
fn visit<'a>(
    index: &'a Index,
    entry: &'a Entry,
    installed: &BTreeSet<String>,
    seen: &mut BTreeSet<String>,
    order: &mut Vec<&'a Entry>,
    missing: &mut Vec<String>,
) {
    if !seen.insert(entry.id.clone()) {
        return;
    }
    for dep in &entry.depends {
        if installed.contains(dep) || seen.contains(dep) {
            continue;
        }
        match index.entry(dep) {
            Some(found) => visit(index, found, installed, seen, order, missing),
            None if !missing.contains(dep) => missing.push(dep.clone()),
            None => {}
        }
    }
    order.push(entry);
}

/// 本机能用的那个文件：写了本机这一种的先，不分系统的其次。
pub(super) fn pick(entry: &Entry) -> Option<&File> {
    let here = Target::here().map(Target::as_str);
    entry
        .files
        .iter()
        .find(|file| file.target.is_some() && file.target.as_deref() == here)
        .or_else(|| entry.files.iter().find(|file| file.target.is_none()))
}

/// 看一眼：要装的那个、一起装的依赖、要下多大。
fn preview(steps: &[Step<'_>], core: &Core) -> Value {
    let Some((last, before)) = steps.split_last() else {
        return Value::Null;
    };
    let entry = last.entry;
    // 和从包文件夹装的看一眼同一个形状（命令行、头照同一套印），带了哪几样照索引摊开；另有下多大、一起装的、来源。
    let mut answer = json!({
        "package": entry.id,
        "version": entry.version,
        "official": entry.official,
        "origin": entry.origin,
        "capabilities": entry.capabilities.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
        "download": steps.iter().map(|step| step.file.size).sum::<u64>(),
        "with": before.iter().map(|step| json!({"package": step.entry.id, "version": step.entry.version})).collect::<Vec<_>>(),
    });
    for (kind, names) in &entry.provides {
        match kind.as_str() {
            "command" => answer["command"] = json!(names.first()),
            "features" => {}
            _ => answer[kind.as_str()] = json!(names),
        }
    }
    let now = core.packages();
    if let Some(old) = now.iter().find(|one| one.id == entry.id) {
        let version = old.read.as_ref().ok().and_then(|old| old.version.clone());
        answer["replaces"] = json!({ "version": version });
    }
    answer
}

/// 推进度给发起的连接：开始、结束各一次，中间最多 [`EVERY`] 一次。她的工具经端口装的没有连接，不推（施工 G-6）。
fn progress<'a>(caller: Option<&'a Reverse>, id: &'a str) -> impl Fn(u64, u64) + Sync + 'a {
    let last: Mutex<Option<Instant>> = Mutex::new(None);
    move |done, total| {
        let Some(caller) = caller else {
            return;
        };
        let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
        let due = done == 0 || done == total || last.is_none_or(|at| at.elapsed() >= EVERY);
        if due {
            *last = Some(Instant::now());
            if !caller.notify(
                "package.progress",
                json!({"package": id, "done": done, "total": total}),
            ) {
                tracing::debug!(target: TARGET, package = id, "package progress not delivered");
            }
        }
    }
}

#[cfg(test)]
mod tests;
