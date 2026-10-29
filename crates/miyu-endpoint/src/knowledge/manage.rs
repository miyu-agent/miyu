//! 人管理知识库（施工 K-7 上，`docs/blueprint/knowledge.md`「怎么走」第八条，`protocol.md` 的 `kb.*`）：建库、改名字说明、删库、
//! 挂目录、加文档、删文档、重建索引。怎么做在 `miyu_store::knowledge`，这里只管读参数、在阻塞线程里调、写回应。照管理员的账号。
//! 加的文档照附件上传好的 blob（2026-10-13 项目主人定）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;
use miyu_store::knowledge::{BaseFields, Knowledge};
use miyu_store::layers::valid;

use super::{blocking, read, report, shown_base};
use crate::Core;
use crate::refusal::Refusal;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Named {
    kb: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    /// 自动想起（施工 K-6 下）：`false` 是拿掉那一格。
    #[serde(default)]
    auto: Option<bool>,
    /// 只有 `kb.mount` 要。
    #[serde(default)]
    folder: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Base {
    kb: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    kb: String,
    path: String,
    blob: String,
    #[serde(default)]
    replace: bool,
}

/// `kb.remove` 的参数（施工 W-14）：`path` 一样，或者 `paths` 几样；每一样是一篇文档或者一个文件夹。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Removal {
    kb: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    paths: Option<Vec<String>>,
}

/// 照方法名办一条管理的 `kb.*`；不是这几个的交回 `None`，由调的一方接着认。
pub(super) async fn call(
    core: &Arc<Core>,
    method: &str,
    params: &Value,
) -> Option<Result<Value, Refusal>> {
    let knowledge = Arc::clone(&core.knowledge);
    Some(match method {
        "kb.create" | "kb.update" | "kb.mount" => named(knowledge, method, params).await,
        "kb.delete" | "kb.reindex" => whole(core, knowledge, method, params).await,
        "kb.add" => document(core, knowledge, params).await,
        "kb.remove" => remove(core, knowledge, params).await,
        _ => return None,
    })
}

/// 建库、改名字说明、挂目录：交回那个库。
async fn named(knowledge: Arc<Knowledge>, method: &str, params: &Value) -> Result<Value, Refusal> {
    let named: Named = read(params)?;
    let mount = method == "kb.mount";
    let renaming = method == "kb.update";
    if !valid(&named.kb)
        || named.folder.is_some() != mount
        || (renaming && named.name.is_none() && named.description.is_none() && named.auto.is_none())
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let (base, skipped) = blocking(move || {
        let fields = BaseFields {
            name: named.name.as_deref(),
            description: named.description.as_deref(),
            auto: named.auto,
        };
        match (named.folder.as_deref(), renaming) {
            (Some(folder), _) => knowledge.mount(&named.kb, folder, fields),
            (None, true) => Ok((knowledge.rename(&named.kb, fields)?, Vec::new())),
            (None, false) => Ok((knowledge.found(&named.kb, fields)?, Vec::new())),
        }
    })
    .await?;
    report(&skipped);
    Ok(shown_base(base))
}

/// 删库（交回回收站里的位置）、重建索引（交回重建完的那个库）。
async fn whole(
    core: &Core,
    knowledge: Arc<Knowledge>,
    method: &str,
    params: &Value,
) -> Result<Value, Refusal> {
    let Base { kb } = read(params)?;
    if !valid(&kb) {
        return Err(Refusal::BAD_PARAMS);
    }
    let home = core.home.clone();
    if method == "kb.delete" {
        let trash = blocking(move || knowledge.delete(&kb, home.as_deref())).await?;
        return Ok(json!({ "trash": trash }));
    }
    let (base, skipped) = blocking(move || knowledge.reindex(&kb)).await?;
    report(&skipped);
    Ok(shown_base(base))
}

/// 加一篇（照上传好的 blob）。
async fn document(
    core: &Core,
    knowledge: Arc<Knowledge>,
    params: &Value,
) -> Result<Value, Refusal> {
    let document: Document = read(params)?;
    if !valid(&document.kb) || !Knowledge::document(&document.path) {
        return Err(Refusal::BAD_PARAMS);
    }
    let blob = document.blob.as_str();
    let blob = ContentHash::parse(blob).map_err(|_| Refusal::BAD_PARAMS)?;
    let blobs = Blobs::new(core.root.blobs());
    let bytes = tokio::task::spawn_blocking(move || crate::attach::read_blob(&blobs, &blob))
        .await
        .map_err(|_| Refusal::INTERNAL)??;
    blocking(move || knowledge.add(&document.kb, &document.path, &bytes, document.replace)).await?;
    Ok(json!({}))
}

/// 删（施工 W-14 起收文件夹、收几样）：`path` 的回 `{"trash": "<回收站里的位置>"}`，同以前删一篇；`paths` 的回
/// `{"trash": [...]}`，照先后。先全查一遍，有一样不在就一样都不删。写法不对、两样都写或都没写、几样重了或者一样在另一样
/// 里面：`bad_params`。
async fn remove(core: &Core, knowledge: Arc<Knowledge>, params: &Value) -> Result<Value, Refusal> {
    let Removal { kb, path, paths } = read(params)?;
    let (single, paths) = match (path, paths) {
        (Some(path), None) => (true, vec![path]),
        (None, Some(paths)) if !paths.is_empty() => (false, paths),
        _ => return Err(Refusal::BAD_PARAMS),
    };
    if !valid(&kb) || !paths.iter().all(|path| Knowledge::shaped(path)) || overlapping(&paths) {
        return Err(Refusal::BAD_PARAMS);
    }
    let home = core.home.clone();
    let trash = blocking(move || knowledge.trash_all(&kb, &paths, home.as_deref())).await?;
    Ok(match single {
        true => json!({ "trash": trash.first() }),
        false => json!({ "trash": trash }),
    })
}

/// 几样里有重了的，或者一样在另一样（文件夹）里面。
fn overlapping(paths: &[String]) -> bool {
    paths.iter().enumerate().any(|(i, one)| {
        paths
            .iter()
            .enumerate()
            .any(|(j, other)| i != j && (one == other || other.starts_with(&format!("{one}/"))))
    })
}
