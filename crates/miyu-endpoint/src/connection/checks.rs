//! 一个请求进来、办之前过的几道（施工 Y-2 上把它们收到一处，设计 06 八再又补第 1 条「每一条路都过同一道检查」）：
//!
//! 1. 凭据决定的闸（施工 W-8、Y-1 上，`gate.rs`）。
//! 2. 本机才能用的（施工 Y-1 下，`reach.rs`）。
//! 3. 扩展照它声明的能力挡（施工 X-30，`rights.rs`）：只对核心拉起的扩展。
//! 4. 会话只给属主（施工 Y-2 上，2026-10-23 项目主人定场所会话只读、不从头上交互）：参数里写了 `session` 的，那个会话的
//!    属主不是这个连接的账号就当没有这个会话（`session_not_found`，看不到的不露，U2：管理员也看不到成员的）。场所会话的属主是
//!    平台包的系统账号：只有那个包自己的连接（桥）碰得到，人的头碰不到。唯一的例外是桥替平台上的主人送话的那个会话（终端管理员
//!    在平台上的私聊，属主是对应表里绑着的本机账号，2026-10-11 核心定）：照 `venue.session` 归属主的同一张对应表放行
//!    （`venues::bridged`）。`blob.get` 不走这一道：它照会话找 blob 有自己更细的一套（`attach/reach.rs`），而且拒的一律
//!    `unknown_blob`，免得拿「会话不存在」和「不是你的」的不同说法探编号。

use std::sync::Arc;

use miyu_kernel::id::{AccountId, SessionId};

use super::{gate, reach, rights};
use crate::Core;
use crate::login::Via;
use crate::refusal::Refusal;
use crate::wire::Request;
use miyu_config::package::Capability;

/// 这个连接是谁、从哪来、扩展声明了什么。
pub(super) struct Who<'a> {
    pub(super) via: Option<&'a Via>,
    pub(super) account: &'a AccountId,
    pub(super) reach: &'a reach::Reach,
    pub(super) declared: &'a [Capability],
}

/// 照先后过几道，第一个拒的交回拒绝；都过了的没有。
pub(super) async fn refuse(core: &Arc<Core>, who: &Who<'_>, request: &Request) -> Option<Refusal> {
    if let Some(refusal) = gate::refuse(core, who.via, who.account, &request.method) {
        return Some(refusal);
    }
    if let Some(refusal) = reach::refuse(who.reach, request) {
        return Some(refusal);
    }
    if who.via == Some(&Via::Spawned)
        && let Some(refusal) = rights::refuse(request, who.declared)
    {
        return Some(refusal);
    }
    others_session(core, who.account, request).await
}

/// 第 4 道：参数里的 `session` 是别人的会话。没写、写法不对、没有这个会话的不在这里拒：方法照它原来的说法答。
async fn others_session(
    core: &Arc<Core>,
    account: &AccountId,
    request: &Request,
) -> Option<Refusal> {
    if request.method == "blob.get" {
        return None;
    }
    let id = request.params.get("session")?.as_str()?;
    let id = SessionId::parse(id).ok()?;
    let (owner, venue) = match core.sessions.loaded(&id).await {
        Some(handle) => (
            handle.owner().clone(),
            Some(handle.venue().as_str().to_string()),
        ),
        None => (core.sessions.owner(core, &id).await?, None),
    };
    if owner == *account || bridged(core, account, &owner, &id, venue).await {
        return None;
    }
    Some(Refusal::NOT_FOUND)
}

/// 系统账号 `account` 的桥能不能碰属主是 `owner` 的会话 `id`：会话的场所是它的平台的、属主是对应表里这个平台的身份对着的
/// 本机账号（同 `venue.sessions` 列的那几个）。场所没交进来的（会话没在跑）照日志第一条读。
async fn bridged(
    core: &Arc<Core>,
    account: &AccountId,
    owner: &AccountId,
    id: &SessionId,
    venue: Option<String>,
) -> bool {
    if !core.is_system(account) {
        return false;
    }
    let venue = match venue {
        Some(venue) => venue,
        None => {
            let dir = core.root.session_dir(owner, id);
            let first =
                tokio::task::spawn_blocking(move || miyu_store::log::first_event(&dir)).await;
            match first {
                Ok(Ok(event)) => match event.body {
                    miyu_kernel::event::Body::SessionCreated(created) => {
                        created.venue.as_str().to_string()
                    }
                    _ => return false,
                },
                _ => return false,
            }
        }
    };
    crate::venues::bridged(core, account, &venue, owner)
}
