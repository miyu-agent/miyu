//! 一个请求进来、办之前过的几道（施工 Y-2 上把它们收到一处，设计 06 八再又补第 1 条「每一条路都过同一道检查」）：
//!
//! 1. 凭据决定的闸（施工 W-8、Y-1 上，`gate.rs`），管理员模式（施工 Y-12 下）。
//! 2. 本机才能用的（施工 Y-1 下，`reach.rs`）。
//! 3. 扩展照它声明的能力挡（施工 X-30，`rights.rs`）：只对核心拉起的扩展。
//! 4. 会话只给碰得到它的（施工 Y-2 上；SU-4 中起照场所认，不照属主）：参数里写了 `session` 的，碰不到就当没有这个会话
//!    （`session_not_found`，看不到的不露）。本机的会话给主人这边的连接，平台桥碰不到；场所会话只给接这个平台的桥（2026-10-23
//!    项目主人定场所会话只读、不从头上交互），主人本人的私聊（没关起来的）另外主人这边的连接也碰得到。`blob.get` 不走这一道：它照
//!    会话找 blob 有自己更细的一套（`attach/reach.rs`），而且拒的一律 `unknown_blob`，免得拿「会话不存在」和「不是你的」的不同
//!    说法探编号。

use std::sync::Arc;

use miyu_kernel::id::SessionId;

use super::{gate, reach, rights};
use crate::Core;
use crate::hello::Caller;
use crate::list::LOCAL;
use crate::login::Via;
use crate::refusal::Refusal;
use crate::wire::Request;
use miyu_config::package::Capability;

/// 这个连接是谁（平台桥照它的包认）、从哪来、扩展声明了什么。
pub(super) struct Who<'a> {
    pub(super) via: Option<&'a Via>,
    pub(super) caller: &'a Caller,
    pub(super) reach: &'a reach::Reach,
    pub(super) declared: &'a [Capability],
}

/// 照先后过几道，第一个拒的交回拒绝；都过了的没有。
pub(super) async fn refuse(core: &Arc<Core>, who: &Who<'_>, request: &Request) -> Option<Refusal> {
    if let Some(refusal) = gate::refuse(who.via, &request.method) {
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
    others_session(core, who.caller, request).await
}

/// 第 4 道：参数里的 `session` 是碰不到的会话。没写、写法不对、没有这个会话的不在这里拒：方法照它原来的说法答。
async fn others_session(core: &Arc<Core>, caller: &Caller, request: &Request) -> Option<Refusal> {
    if request.method == "blob.get" {
        return None;
    }
    let id = request.params.get("session")?.as_str()?;
    let id = SessionId::parse(id).ok()?;
    let owner = match core.sessions.loaded(&id).await {
        Some(handle) => handle.owner().clone(),
        None => core.sessions.owner(core, &id).await?,
    };
    let (venue, confined) = crate::sessions::placed(core, &owner, &id).await?;
    let reachable = match crate::venues::bridge(core, caller) {
        Some(bridge) => bridge.platforms(core).holds(venue.as_str()),
        None => owner == core.admin && (venue.as_str() == LOCAL || !confined),
    };
    (!reachable).then_some(Refusal::NOT_FOUND)
}
