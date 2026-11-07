//! 会话关不关在沙盒里（施工 5-12；施工 SU-4 中起照 `session.created` 记的 `confined`）：造的时候算、之后照日志第一条读。

use miyu_kernel::event::Body;
use miyu_kernel::id::{SessionId, VenueId};
use miyu_store::log::first_event;

use crate::Core;
use crate::hello::Caller;
use crate::list::LOCAL;

/// 会话 `id` 关没关着（施工 SU-4 中）：照日志第一条 `session.created` 的 `confined`。读不了的算关着。
pub(crate) async fn confined_of(core: &Core, id: &SessionId) -> bool {
    placed(core, id).await.is_none_or(|(_, confined)| confined)
}

/// 会话 `id` 的场所、关没关着（同 [`confined_of`]）：照日志第一条读，读不了的没有。
pub(crate) async fn placed(core: &Core, id: &SessionId) -> Option<(VenueId, bool)> {
    let dir = core.root.session_dir(id);
    let first = tokio::task::spawn_blocking(move || first_event(&dir)).await;
    match first {
        Ok(Ok(event)) => match event.body {
            Body::SessionCreated(created) => {
                let shut = created.confined;
                Some((created.venue, shut))
            }
            _ => None,
        },
        _ => None,
    }
}

/// 有这个会话 `id` 时，`caller` 碰不碰得到它（施工 Y-2 上的第 4 道，`connection/checks.rs`；施工 AS-3 抽出来，`schedule.add`
/// 的 `target.session` 也照它判）：本机的会话给主人这边的连接，平台桥碰不到；场所会话只给接这个平台的桥，主人本人的私聊
/// （没关起来的）另外主人这边的连接也碰得到。读不到场所的不在这里拒，照调的一方原来的说法答。
pub(crate) async fn reachable(core: &Core, caller: &Caller, id: &SessionId) -> bool {
    let Some((venue, confined)) = placed(core, id).await else {
        return true;
    };
    reaches(core, caller.package.as_deref(), venue.as_str(), confined)
}

/// 包 `package` 的连接（人的头没有包）碰不碰得到场所是 `venue`、关没关着是 `confined` 的会话：规矩同 [`reachable`]。发
/// `tool.after` 照它挑收件的扩展（施工 HK-2，设计 65 第四节第 5 条：和订事件流一模一样）。
pub(crate) fn reaches(core: &Core, package: Option<&str>, venue: &str, confined: bool) -> bool {
    match package.and_then(|package| crate::venues::bridge_of(core, package)) {
        Some(bridge) => bridge.platforms(core).holds(venue),
        None => venue == LOCAL || !confined,
    }
}
