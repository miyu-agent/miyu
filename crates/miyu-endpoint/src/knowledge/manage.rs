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
    /// 只有 `kb.add` 要。
    #[serde(default)]
    blob: Option<String>,
    #[serde(default)]
    replace: bool,
}

/// 照方法名办一条管理的 `kb.*`；不是这几个的交回 `None`，由调的一方接着认。
pub(super) async fn call(
    core: &Arc<Core>,
    method: &str,
    params: &Value,
) -> Option<Result<Value, Refusal>> {
    let knowledge = Arc::clone(&core.knowledge);
    Some(match method {
        "kb.create" | "kb.update" | "kb.mount" => named(core, knowledge, method, params).await,
        "kb.delete" | "kb.reindex" => whole(core, knowledge, method, params).await,
        "kb.add" | "kb.remove" => document(core, knowledge, method, params).await,
        _ => return None,
    })
}

/// 建库、改名字说明、挂目录：交回那个库。
async fn named(
    core: &Core,
    knowledge: Arc<Knowledge>,
    method: &str,
    params: &Value,
) -> Result<Value, Refusal> {
    let named: Named = read(params)?;
    let mount = method == "kb.mount";
    let renaming = method == "kb.update";
    if !valid(&named.kb)
        || named.folder.is_some() != mount
        || (renaming && named.name.is_none() && named.description.is_none() && named.auto.is_none())
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let account = core.admin.clone();
    let (base, skipped) = blocking(move || {
        let fields = BaseFields {
            name: named.name.as_deref(),
            description: named.description.as_deref(),
            auto: named.auto,
        };
        match (named.folder.as_deref(), renaming) {
            (Some(folder), _) => knowledge.mount(&account, &named.kb, folder, fields),
            (None, true) => Ok((knowledge.rename(&account, &named.kb, fields)?, Vec::new())),
            (None, false) => Ok((knowledge.found(&account, &named.kb, fields)?, Vec::new())),
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
    let (account, home) = (core.admin.clone(), core.home.clone());
    if method == "kb.delete" {
        let trash = blocking(move || knowledge.delete(&account, &kb, home.as_deref())).await?;
        return Ok(json!({ "trash": trash }));
    }
    let (base, skipped) = blocking(move || knowledge.reindex(&account, &kb)).await?;
    report(&skipped);
    Ok(shown_base(base))
}

/// 加一篇（照上传好的 blob）、删一篇（挪进回收站，交回回收站里的位置）。
async fn document(
    core: &Core,
    knowledge: Arc<Knowledge>,
    method: &str,
    params: &Value,
) -> Result<Value, Refusal> {
    let document: Document = read(params)?;
    let adding = method == "kb.add";
    if !valid(&document.kb)
        || !Knowledge::document(&document.path)
        || document.blob.is_some() != adding
        || (document.replace && !adding)
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let (account, home) = (core.admin.clone(), core.home.clone());
    let Some(blob) = document.blob.as_deref() else {
        let (_, trash) = blocking(move || {
            knowledge.trash(&account, &document.kb, &document.path, home.as_deref())
        })
        .await?;
        return Ok(json!({ "trash": trash }));
    };
    let blob = ContentHash::parse(blob).map_err(|_| Refusal::BAD_PARAMS)?;
    let blobs = Blobs::new(core.root.blobs(&core.admin));
    let bytes = tokio::task::spawn_blocking(move || crate::attach::read_blob(&blobs, &blob))
        .await
        .map_err(|_| Refusal::INTERNAL)??;
    blocking(move || {
        knowledge.add(
            &account,
            &document.kb,
            &document.path,
            &bytes,
            document.replace,
        )
    })
    .await?;
    Ok(json!({}))
}
