//! `blob.get` 带 `session`（施工 W-6 补，`protocol.md` 的 `blob.get` 第 1 条）：照那个会话的属主找 blob，不照连接的账号。
//! 接入QQ 的会话 2026-10-17 要的：群会话归系统账号，`show` 存的 blob 在它名下；以前一律照管理员找，桥读不到。
//!
//! 谁读得到：会话的属主（施工 Y-2 上起管理员不再算：U2，管理员也看不到成员的会话；场所会话只在平台的后台看）；别的系统账号只读得到它的平台的场所会话（同 `venue.sessions`
//! 列谁的会话，`venues::owners`）里引用过的 blob：消息、工具结果的图片块、文件块，`file.shown` 的 blob，她经工具发出去的图
//! （`tool.result.sent`，施工 W-6 再补：接入QQ 发表情包要读管理员私聊里的）（[`references`]）。只认
//! 结构里的引用，不在原文里搜：桥能以外部身份往主人的私聊里发字，照原文搜它就能写一句带着哈希的话、读走主人别的 blob。
//! 不合的一律 `unknown_blob`，不说是哪一样不对：免得拿它探某个 blob 在不在。
//!
//! 扩展的连接（施工 W-6 三补）：照账号放行的那一条（属主）不放它，它要的 blob 也得是那个会话引用过的。没声明系统账号
//! 的包的扩展替管理员干活（施工 Y-5 中上起身份是包自己的账号），以前写上管理员随便一个会话，就读得到管理员名下任何一个 blob。

use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::{AccountId, ContentHash, SessionId};

use super::blocking;
use crate::Core;
use crate::refusal::Refusal;

/// 照会话 `session` 找 blob `hash` 时，去哪个账号名下读；`caller` 是连接的账号（身份：桥认它），`serving` 是它替谁干活（属主照它
/// 比，施工 Y-5 中上），`extension` 是不是扩展的连接。会话编号不合
/// 写法的 `bad_params`。
pub(super) async fn owner(
    core: &Core,
    (caller, serving, extension): (&AccountId, &AccountId, bool),
    session: &str,
    hash: &ContentHash,
) -> Result<AccountId, Refusal> {
    let id = SessionId::parse(session).map_err(|_| Refusal::BAD_PARAMS)?;
    let owner = core
        .sessions
        .owner(core, &id)
        .await
        .ok_or(Refusal::UNKNOWN_BLOB)?;
    let own = *serving == owner;
    if own && !extension {
        return Ok(owner);
    }
    let owners = (!own).then(|| crate::venues::owners(core, caller));
    let dir = core.root.session_dir(&owner, &id);
    let (hash, reached) = (hash.clone(), owner.clone());
    blocking(move || {
        if let Some(owners) = owners {
            let venue = match miyu_store::log::first_event(&dir).map(|event| event.body) {
                Ok(Body::SessionCreated(created)) => created.venue,
                _ => return Err(Refusal::UNKNOWN_BLOB),
            };
            if !owners.take(venue.as_str(), &reached) {
                return Err(Refusal::UNKNOWN_BLOB);
            }
        }
        let mut found = false;
        let read = miyu_store::log::read_segments(&dir, |events| {
            found = events.iter().any(|event| references(event, &hash));
            !found
        });
        match read.is_ok() && found {
            true => Ok(reached),
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
