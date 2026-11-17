//! 人生那一行这一轮在不在跑（施工 LF-15，`docs/blueprint/life.md`「人生会话里的 `/loop`、`/stop`」第 1 条）：头的人生卡要写准
//! 「这一轮正在跑」还是「下次继续几点」，定时表的列表、推送里人生那一行在跑的多带 `busy: true`。
//!
//! 会话忙不忙变了（会话表端口的 `listing`，`spawn.rs`）就来看一眼：它是人生会话的（内存里那一份人生的行里有它），问会话表
//! 这一刻忙不忙，和记着的不一样就记下、推一次那一行（`Schedules::notify`）。不是人生会话的当场交回，不问、不推。只在内存里：
//! 核心重启以后没有在跑的回合，空着就是对的。

use std::sync::Arc;

use miyu_kernel::id::SessionId;

use crate::Core;

/// 会话 `session` 的状态变了：它是人生会话的，照这一刻忙不忙记下，变了的推一次那一行。问会话表要等一把锁，放在自己的任务里。
pub(crate) fn follow(core: &Arc<Core>, session: SessionId) {
    let Some(id) = core.schedules().life_row(&session) else {
        return;
    };
    let core = Arc::clone(core);
    tokio::spawn(async move {
        let busy = core.sessions.busy_ids().await.contains(&session);
        if core.schedules().round(&session, busy) {
            core.schedules().notify(&id);
        }
    });
}
