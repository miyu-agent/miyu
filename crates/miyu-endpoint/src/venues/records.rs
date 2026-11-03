//! `venue.records {session, msg, count}`（施工 O-24，`docs/construction/O-24-判官看的群聊记录.md`）：判官看的群聊记录，回应
//! `{"records": "…", "current": "…"}`。照会话日志读（同 `view.page`，不载入会话），留着一切的有效历史（撤掉的、撤回的不算，压缩
//! 换出去的照样算），照会话快照里钉下的时区和字渲染（群会话）；快照里没有的（私聊）照核心这时的时区和出厂的字。
//! `count` 1 到 100；`msg` 不是带 `venue` 的 `message.user` 的 `bad_params`。写了 `grow`（施工 O-44）的，起点照步长对齐，条数在
//! `count` 到 `count + grow − 1` 之间（[`miyu_assemble::group::Window`]）；`grow` 至少 1，`count + grow` 不过 100。回应另带
//! `count`：之前的记录实际有几条。
//!
//! 边读边记（施工 V-39 上，`read.rs`）：不再把整份日志读成事件、整份交给有效历史、一万多行全渲染再挑；撤销过的照样交给真的
//! 有效历史算，前面谁都碰不到的那一截收下就放掉。看到的字、报错和先后不变。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{Seq, SessionId};
use miyu_policy::{ForwardTexts, GroupChat, Snapshot};
use miyu_store::blob::Blobs;
use miyu_store::resources::ResourceRoot;

use crate::Core;
use crate::refusal::Refusal;

mod pass;
mod read;

use crate::sweep::Tuning;

/// `count` 最多几条。
const MOST: u64 = 100;

/// `venue.records` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordsParams {
    session: String,
    msg: u64,
    count: u64,
    /// 起点对齐的步长（施工 O-44）；不写的是最近的 `count` 条。
    #[serde(default)]
    grow: Option<u64>,
}

/// `venue.records`：见模块的说明。
pub(crate) async fn records(core: &Core, params: RecordsParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let msg = Seq::new(params.msg).ok_or(Refusal::BAD_PARAMS)?;
    if !(1..=MOST).contains(&params.count) {
        return Err(Refusal::BAD_PARAMS);
    }
    let count = usize::try_from(params.count).map_err(|_| Refusal::BAD_PARAMS)?;
    let grow = match params.grow {
        None => None,
        Some(grow) if grow >= 1 && params.count + grow <= MOST => {
            Some(usize::try_from(grow).map_err(|_| Refusal::BAD_PARAMS)?)
        }
        Some(_) => return Err(Refusal::BAD_PARAMS),
    };
    let window = miyu_assemble::group::Window { count, grow };
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let dir = core.root.session_dir(&session);
    let blobs = Blobs::new(core.root.blobs());
    let resources = core.resources.clone();
    let offset = crate::sessions::offset().minutes();
    let rendered = tokio::task::spawn_blocking(move || {
        let texts = |first: Option<&Event>| texts(first, &blobs, &resources, offset);
        read::judge(&mut read::Dir(dir), msg, window, texts, Tuning::SHIPPED)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    Ok(json!({"records": rendered.records, "current": rendered.current, "count": rendered.count}))
}

/// 照哪份字渲染（日志的第一条是 `first`）：群会话照快照里钉下的；没有的照核心这时的时区和出厂的字。合并转发那几行的字
/// （施工 O-34）同样：群会话照快照的 `core.forward`，O-34 以前造的没有就不写（和她看到的一样）；快照里没有群会话的字的照
/// 这时的。
fn texts(
    first: Option<&Event>,
    blobs: &Blobs,
    resources: &ResourceRoot,
    offset: i32,
) -> Result<
    (
        miyu_assemble::GroupChat,
        Option<miyu_assemble::ForwardTexts>,
    ),
    Refusal,
> {
    let pinned = match first.map(|event| &event.body) {
        Some(Body::SessionCreated(created)) => blobs
            .get(&created.policy)
            .ok()
            .and_then(|bytes| Snapshot::from_bytes(&bytes).ok())
            .and_then(|snapshot| snapshot.group.map(|group| (group, snapshot.core.forward))),
        _ => None,
    };
    let unread = |error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "group texts not read");
        Refusal::INTERNAL
    };
    let (chat, forward) = pick(pinned, || {
        Ok((
            resources.group_chat(offset).map_err(unread)?,
            Some(resources.forward().map_err(unread)?),
        ))
    })?;
    let texts = chat.texts().map_err(|_| Refusal::INTERNAL)?;
    let forward = forward
        .as_ref()
        .map(ForwardTexts::texts)
        .transpose()
        .map_err(|_| Refusal::INTERNAL)?;
    Ok((texts, forward))
}

/// 群会话钉下的字和转发那几行的字（施工 O-34）。
type Pinned = (GroupChat, Option<ForwardTexts>);

/// 渲染照哪一份：群会话快照里钉下的（`pinned`），没有的照这时的（`now`）。
fn pick(
    pinned: Option<Pinned>,
    now: impl FnOnce() -> Result<Pinned, Refusal>,
) -> Result<Pinned, Refusal> {
    match pinned {
        Some(chat) => Ok(chat),
        None => now(),
    }
}

#[cfg(test)]
mod tests;
