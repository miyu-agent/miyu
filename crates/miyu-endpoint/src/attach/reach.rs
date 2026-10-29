//! `blob.get` 带 `session`（施工 W-6 补，`protocol.md` 的 `blob.get` 第 1 条）：照那个会话引用过的找 blob。
//!
//! 谁读得到（施工 SU-4 中起照场所认，同 `connection/checks.rs` 的第 4 道）：本机的会话、主人本人的私聊（没关起来的），主人这边
//! 的人的连接随便读；扩展的连接（施工 W-6 三补）要的 blob 还得是那个会话引用过的；平台桥只读得到它的平台的场所会话里引用过的
//! blob：消息、工具结果的图片块、文件块，`file.shown` 的 blob，她经工具发出去的图（`tool.result.sent`，施工 W-6 再补：接入QQ 发
//! 表情包要读主人私聊里的）（[`references`]）。只认结构里的引用，不在原文里搜：桥能以外部身份往主人的私聊里发字，照原文搜它就能
//! 写一句带着哈希的话、读走主人别的 blob。不合的一律 `unknown_blob`，不说是哪一样不对：免得拿它探某个 blob 在不在。

use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::{ContentHash, SessionId};

use super::blocking;
use crate::Core;
use crate::hello::Caller;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// 照会话 `session` 找 blob `hash` 读不读得到；`caller` 是连接（平台桥照它的包认），`extension` 是不是扩展的连接。会话编号
/// 不合写法的 `bad_params`。
pub(super) async fn check(
    core: &Core,
    (caller, extension): (&Caller, bool),
    session: &str,
    hash: &ContentHash,
) -> Result<(), Refusal> {
    let id = SessionId::parse(session).map_err(|_| Refusal::BAD_PARAMS)?;
    if !core.sessions.exists(core, &id).await {
        return Err(Refusal::UNKNOWN_BLOB);
    }
    let (venue, confined) = crate::sessions::placed(core, &id)
        .await
        .ok_or(Refusal::UNKNOWN_BLOB)?;
    let reachable = match crate::venues::bridge(core, caller) {
        Some(bridge) => bridge.platforms(core).holds(venue.as_str()),
        None => venue.as_str() == LOCAL || !confined,
    };
    if !reachable {
        return Err(Refusal::UNKNOWN_BLOB);
    }
    if !extension {
        return Ok(());
    }
    let dir = core.root.session_dir(&id);
    let hash = hash.clone();
    blocking(move || {
        let mut found = false;
        let read = miyu_store::log::read_segments(&dir, |events| {
            found = events.iter().any(|event| references(event, &hash));
            !found
        });
        match read.is_ok() && found {
            true => Ok(()),
            false => Err(Refusal::UNKNOWN_BLOB),
        }
    })
    .await
}

/// 一条事件引用了 blob `hash`：消息、工具结果的图片块、文件块（`attach::blobs_of`），工具结果的 `file.shown`、`sent`。
fn references(event: &Event, hash: &ContentHash) -> bool {
    let blocks = match &event.body {
        Body::MessageUser(message) => &message.blocks,
        Body::MessageAssistant(reply) => &reply.blocks,
        Body::ToolResult(result) => {
            if result
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::FileShown(shown) if shown.blob == *hash))
                || result.sent.iter().any(|sent| sent.image.blob == *hash)
            {
                return true;
            }
            &result.blocks
        }
        _ => return false,
    };
    super::blobs_of(blocks).contains(hash)
}

#[cfg(test)]
mod tests;
