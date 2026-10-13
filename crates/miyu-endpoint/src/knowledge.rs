//! 人经协议查知识库（施工 K-1 上，`docs/blueprint/knowledge.md`「对外的样子」，`protocol.md` 的 `kb.*`）：列、搜、读。怎么补
//! 索引、怎么搜在 `miyu_store::knowledge`，这里只管读参数、在阻塞线程里调、写回应、把跳过的文档记一行运行日志。照管理员的
//! 账号（和 `memory.*` 一样）。不看知识库这个软件包装没装：文件是人的，人随时看得到。

mod manage;

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use miyu_recall::associate::Thresholds;
use miyu_store::knowledge::{Base, Hit, Knowledge, KnowledgeError, Meaning, Skipped};
use miyu_store::layers::valid;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;

/// 核心一份的知识库：`~/` 照家目录 `home` 接（施工 K-4），PDF、网页照抽字的小程序 `extractor` 抽（施工 K-5），只照意思找到的
/// 那一路的下限照资源目录 `resources` 里的表（施工 K-2 修，`core/knowledge/search.toml`）：读不出来的记一行，都照
/// [`miyu_recall::FLOOR`]，别的照常。
pub(crate) fn shelf(
    root: &DataRoot,
    home: Option<PathBuf>,
    extractor: Option<PathBuf>,
    resources: &ResourceRoot,
) -> Arc<Knowledge> {
    let floors = resources
        .knowledge_table("search")
        .map_err(|error| error.to_string())
        .and_then(|text| Thresholds::parse(&text))
        .inspect_err(|error| {
            tracing::warn!(target: "miyu::endpoint", error = error.as_str(), "knowledge search floors unreadable");
        })
        .unwrap_or_default();
    Arc::new(
        Knowledge::new(root)
            .with_home(home)
            .with_extractor(extractor)
            .with_floors(floors),
    )
}

/// 主程序真实位置旁边的抽字的小程序 `miyu-extract`（施工 K-5，`knowledge.md`「怎么走」第二条）：找不到的没有，那样的核心不认
/// PDF、网页。
pub(crate) fn extractor() -> Option<PathBuf> {
    let exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .ok()?;
    miyu_store::knowledge::extractor_beside(&exe)
}

impl Core {
    /// 同一份家底，知识库抽 PDF、网页照 `extractor` 这个小程序（施工 K-5；测试里交 cargo 编出来的那个，核心自己照主程序旁边
    /// 找）。
    #[must_use]
    pub fn with_extractor(mut self, extractor: Option<PathBuf>) -> Core {
        self.knowledge = shelf(&self.root, self.home.clone(), extractor, &self.resources);
        self
    }
}

/// `kb.search` 不写 `limit` 时给几块。
const SEARCH: usize = 10;
/// `limit` 最多写几。
const LIMIT: usize = 50;

#[derive(Debug, Deserialize)]
struct Search {
    query: String,
    #[serde(default)]
    kb: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct Read {
    kb: String,
    path: String,
    #[serde(default)]
    page: Option<usize>,
}

/// 照方法名办一条 `kb.*`（`methods.rs` 把 `kb.` 开头的都转过来，不认识的回 `unknown_method`）。
pub(crate) async fn call(core: &Arc<Core>, method: &str, params: &Value) -> Result<Value, Refusal> {
    if let Some(managed) = manage::call(core, method, params).await {
        return managed;
    }
    let knowledge = Arc::clone(&core.knowledge);
    let account = core.admin.clone();
    match method {
        "kb.list" => {
            let (bases, skipped) = blocking(move || knowledge.list(&account)).await?;
            report(&skipped);
            let bases: Vec<Value> = bases.into_iter().map(shown_base).collect();
            Ok(json!({ "bases": bases }))
        }
        "kb.search" => {
            let search: Search = read(params)?;
            let limit = search.limit.unwrap_or(SEARCH);
            if search.query.trim().is_empty()
                || !(1..=LIMIT).contains(&limit)
                || search.kb.as_deref().is_some_and(|kb| !valid(kb))
            {
                return Err(Refusal::BAD_PARAMS);
            }
            // 照意思找（施工 K-2）：问句照这时的配置算向量，搜完在后台补搜过的几个库缺的向量；同她的 `kb_search`。
            let using = crate::memory::using(core);
            let vectors = core.memory.vectors();
            let near = match &vectors {
                Some(vectors) => vectors.query(&using, &search.query).await,
                None => None,
            };
            let searching = Arc::clone(&knowledge);
            let (hits, skipped, searched) = blocking(move || {
                let only = search.kb.map(|kb| vec![kb]);
                let meaning = near.as_ref().map(|near| Meaning {
                    model: &near.model,
                    vector: &near.vector,
                });
                let (hits, skipped) =
                    searching.search(&account, &search.query, only.as_deref(), limit, meaning)?;
                let searched = match only {
                    Some(only) => only,
                    None => searching.ids(&account)?,
                };
                Ok((hits, skipped, searched))
            })
            .await?;
            report(&skipped);
            if let Some(vectors) = vectors {
                for id in searched {
                    let index = knowledge.index_of(&core.admin, &id);
                    vectors.fill_knowledge(&using, format!("kb {} {id}", core.admin), index);
                }
            }
            Ok(json!({ "hits": hits.iter().map(shown).collect::<Vec<Value>>() }))
        }
        "kb.read" => {
            let read: Read = read(params)?;
            let page = read.page.unwrap_or(1);
            if !valid(&read.kb) || page == 0 {
                return Err(Refusal::BAD_PARAMS);
            }
            let page =
                blocking(move || knowledge.read(&account, &read.kb, &read.path, page)).await?;
            Ok(json!({ "text": page.text, "page": page.page, "pages": page.pages }))
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 一个库在回应里的样子（`kb.list` 的一项；建、改、挂、重建的回应，施工 K-7 上）。
fn shown_base(base: Base) -> Value {
    let mut value = json!({
        "id": base.id,
        "name": base.name,
        "description": base.description,
        "documents": base.documents,
        "chunks": base.chunks,
        "auto": base.auto,
    });
    if let Some(error) = base.error {
        value["error"] = Value::String(error);
    }
    value
}

/// 一块在回应里的样子。
fn shown(hit: &Hit) -> Value {
    let mut shown = json!({
        "kb": hit.kb,
        "path": hit.path,
        "chunk": hit.chunk,
        "heading": hit.heading,
        "text": hit.text,
        "at": hit.at.to_string(),
    });
    // PDF 的那一块带第几页（施工 K-5），别的不写这一格。
    if let Some(page) = hit.page {
        shown["page"] = json!(page);
    }
    shown
}

/// 跳过的文档记一行（`knowledge.md`「怎么走」第三条第 4 款）。
fn report(skipped: &[Skipped]) {
    for one in skipped {
        tracing::warn!(target: "miyu::endpoint", kb = one.kb.as_str(), path = one.path.as_str(), why = one.why.as_str(), "knowledge document skipped");
    }
}

fn read<T: DeserializeOwned>(params: &Value) -> Result<T, Refusal> {
    serde_json::from_value(params.clone()).map_err(|_| Refusal::BAD_PARAMS)
}

/// 在阻塞线程里做，[`KnowledgeError`] 写成拒绝：没有的库 `unknown_kb`，没有的文档 `unknown_document`，没有这一页
/// `bad_params`；管理的几种（施工 K-7 上）：已经有的库 `kb_exists`、已经有的文档 `document_exists`、挂进来的 `kb_read_only`、
/// 挂不上的目录 `folder_unavailable`、挪不进回收站 `trash_failed`；读不了的记一行、是内部出错。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, KnowledgeError> + Send + 'static,
) -> Result<T, Refusal> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(KnowledgeError::UnknownBase)) => Err(Refusal::UNKNOWN_KB),
        Ok(Err(KnowledgeError::UnknownDocument)) => Err(Refusal::UNKNOWN_DOCUMENT),
        Ok(Err(KnowledgeError::BadPage)) => Err(Refusal::BAD_PARAMS),
        Ok(Err(KnowledgeError::BaseExists)) => Err(Refusal::KB_EXISTS),
        Ok(Err(KnowledgeError::Exists)) => Err(Refusal::DOCUMENT_EXISTS),
        Ok(Err(KnowledgeError::ReadOnly)) => Err(Refusal::KB_READ_ONLY),
        Ok(Err(KnowledgeError::FolderUnavailable(why))) => Err(Refusal::folder_unavailable(why)),
        Ok(Err(KnowledgeError::Trash(why))) => Err(Refusal::trash_failed(why)),
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "knowledge failed");
            Err(Refusal::INTERNAL)
        }
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "knowledge failed");
            Err(Refusal::INTERNAL)
        }
    }
}

#[cfg(test)]
mod tests;
