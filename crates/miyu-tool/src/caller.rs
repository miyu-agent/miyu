//! 调用的事实（施工 X-43，`docs/blueprint/skills.md`「脚本」第 2、3 条）：这一次在哪种场所、这一轮开口的是不是机器主人。只是
//! 事实，不是门：脚本照它调整行为（比如群里只读、不改东西），挡场所的是脚本开头的 `Venues`。

use miyu_kernel::id::VenueId;

/// 调用的事实：执行器照派它的那一轮造，脚本拿到的是环境变量 `MIYU_CALLER_VENUE`、`MIYU_CALLER_OWNER`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    /// 会话的场所：本机的是 `local`，场所会话是场所编号（`qq:group:123` 这样）。
    pub venue: VenueId,
    /// 这一轮开口的人是不是机器主人：本机的会话是；场所会话照这一轮的 `speakers`，有开口的人、每个都是主人才是。拿不到的
    /// （回合中途核心重启、载入以后）不是。
    pub owner: bool,
}
