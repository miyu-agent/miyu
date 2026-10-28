//! `view.detail`：一次调用的完整差异（施工 9-6 三补，`docs/blueprint/protocol.md`「`view.detail`」，`04-核心协议.md` 第九节
//! 「条目只带摘要，完整的 diff 头展开时按需取」）。照调用编号找那次调用的工具结果，照它的效果一个文件一项：改了的从属主的
//! blob 拿改前改后算差异（[`crate::diffs`]，不读磁盘：之后又被改过的照样是那一次的），移进回收站的只写一项。施工 9-8 中
//! 多交结果原文 `output`，另认 `compaction` 交那一次压缩的摘要；字段只加不改。施工 M-5 上多交 `nested`：挂在这次调用、
//! 这个任务（另认 `job`）下面的嵌套调用，整份或 `nested_after` 以后的。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::Block as Content;
use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::{AccountId, CallId, JobId, Parent, SessionId};
use miyu_store::blob::Blobs;
use miyu_view::NestedItem;

use crate::Core;
use crate::diffs;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `view.detail` 的参数：`call`、`job`、`compaction` 正好写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DetailParams {
    /// 会话编号。
    session: String,
    /// 那一次工具调用的编号。
    #[serde(default)]
    call: Option<String>,
    /// 那一次压缩替代到的序号：压缩那一条的编号 `c<序号>` 里的数（施工 9-8 中）。
    #[serde(default)]
    compaction: Option<u64>,
    /// 后台任务的编号（施工 M-5 上）：交它下面的嵌套调用。
    #[serde(default)]
    job: Option<String>,
    /// 只要序号比它大的嵌套调用，加上它以前还在跑的（施工 M-5 上，网页要的）。
    #[serde(default)]
    nested_after: Option<u32>,
}

/// `view.detail`：调用的回应 `{"files": [...], "output"}`，`files` 照那次调用的效果的先后，`output` 是结果原文（施工 9-8 中）；
/// 压缩的回应 `{"summary"}`，检查点的摘要原文。
pub(crate) async fn detail(
    core: &Core,
    peer: Peer,
    params: DetailParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let after = params.nested_after;
    match (params.call, params.job, params.compaction) {
        (Some(call), None, None) => {
            let call = CallId::parse(&call).map_err(|_| Refusal::BAD_PARAMS)?;
            call_detail(core, peer, &session, call, after).await
        }
        (None, Some(job), None) => {
            let job = JobId::parse(&job).map_err(|_| Refusal::BAD_PARAMS)?;
            let (owner, events) = super::events_of(core, &session).await?;
            let nested = nested(
                core,
                peer,
                (&session, &owner),
                &events,
                Parent::Job(job),
                after,
            )
            .await?;
            Ok(json!({ "nested": nested }))
        }
        (None, None, Some(upto)) => compaction(core, &session, upto).await,
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 一次调用：改了的文件、结果原文、挂在下面的嵌套调用（有的才写）。还没结果的、下面调过工具的只交 `nested`。
async fn call_detail(
    core: &Core,
    peer: Peer,
    session: &SessionId,
    call: CallId,
    after: Option<u32>,
) -> Result<Value, Refusal> {
    let (owner, events) = super::events_of(core, session).await?;
    let nested = nested(
        core,
        peer,
        (session, &owner),
        &events,
        Parent::Call(call),
        after,
    )
    .await?;
    let result = events.into_iter().rev().find_map(|event| match event.body {
        Body::ToolResult(result) if result.call_id == call => Some(result),
        _ => None,
    });
    let Some(result) = result else {
        return match nested.is_empty() {
            true => Err(Refusal::UNKNOWN_CALL),
            false => Ok(json!({ "nested": nested })),
        };
    };
    let output = output(&result.blocks);
    let blobs = Blobs::new(core.root.blobs());
    let files = tokio::task::spawn_blocking(move || {
        result
            .effects
            .iter()
            .filter_map(|effect| file(&blobs, effect))
            .collect::<Vec<Value>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let mut reply = json!({ "files": files, "output": output });
    if !nested.is_empty() {
        reply["nested"] = json!(nested);
    }
    Ok(reply)
}

/// 挂在 `parent` 下面的嵌套调用（施工 M-5 上）：日志里了结的加上会话这一刻在跑的（载入着才有），标题照连接的语言、技能的名字
/// 照会话属主 `owner` 看得到的（施工 U23 上下）。一个都没有的不读字。
async fn nested(
    core: &Core,
    peer: Peer,
    (session, owner): (&SessionId, &AccountId),
    events: &[Event],
    parent: Parent,
    after: Option<u32>,
) -> Result<Vec<NestedItem>, Refusal> {
    let running = match core.sessions.loaded(session).await {
        Some(handle) => handle
            .current()
            .await
            .map(|current| current.nested)
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let any = running.iter().any(|call| *call.id.parent() == parent)
        || events.iter().any(|event| {
            matches!(&event.body, Body::ToolNested(nested) if *nested.id.parent() == parent)
        });
    if !any {
        return Ok(Vec::new());
    }
    // 读字碰磁盘，同 `view.page` 拿到阻塞线程里。
    let (resources, titles, language) = (
        core.resources.clone(),
        super::Titles::of(core, owner),
        peer.language,
    );
    let texts = tokio::task::spawn_blocking(move || super::texts(&resources, language, &titles))
        .await
        .map_err(|_| Refusal::INTERNAL)??;
    Ok(miyu_view::listed(&parent, events, &running, after, &texts))
}

/// 一次压缩：替代到 `upto` 的那个检查点的摘要原文。没有的照没有这次调用拒。
async fn compaction(core: &Core, session: &SessionId, upto: u64) -> Result<Value, Refusal> {
    let (_, events) = super::events_of(core, session).await?;
    let summary = events
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            Body::ContextCompacted(compacted) if compacted.upto.get() == upto => {
                Some(compacted.summary)
            }
            _ => None,
        })
        .ok_or(Refusal::UNKNOWN_CALL)?;
    Ok(json!({ "summary": summary }))
}

/// 结果原文：给模型看的文字块，照先后接起来，块之间空一行；图片在条目的 `images` 里，不进这里。
fn output(blocks: &[Content]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            Content::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("\n\n")
}

/// 一个效果写成的一项：改了的带差异（算不出的说原因），移进回收站的只写一项，别的（读文件、派任务）不算。
fn file(blobs: &Blobs, effect: &Effect) -> Option<Value> {
    match effect {
        Effect::FileChanged(changed) => {
            let mut item = json!({"path": changed.path, "action": "write"});
            let whole =
                diffs::sides(blobs, changed).and_then(|(then, now)| diffs::unified(&then, &now));
            match whole {
                Ok(whole) => {
                    item["diff"] = json!(whole.lines);
                    item["added"] = json!(whole.added);
                    item["removed"] = json!(whole.removed);
                }
                Err(skipped) => item["skipped"] = json!(skipped.as_str()),
            }
            Some(item)
        }
        Effect::FileTrashed(trashed) => Some(json!({"path": trashed.path, "action": "trash"})),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
