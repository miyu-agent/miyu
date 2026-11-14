//! 交给工具的人生端口（施工 LF-4 下，`docs/blueprint/life.md`「结束」）：她的 `end_life` 经它调 [`super::finish()`]，`by` 是那次
//! 工具调用，带遗言。随定时端口交进会话（[`miyu_tool::SchedulePort::life`]），一个会话一份。
//!
//! - 记下 `life.ended` 就答，后面的清理不等：清理要等这一轮结束、会话封存，这一轮又在等工具答，等了就卡死。清理在
//!   `finish` 自己的任务里做完，没成的记一行，主人再结束一次（`life.kill`）接着清完。
//! - 同一个会话里几次一起调的排着来：先到的记下，后到的看到已经结束了，不重复记、不重复推通知。

use std::sync::{Arc, Weak};

use tokio::sync::Mutex;

use miyu_kernel::id::{CallId, SessionId};
use miyu_kernel::origin::{By, Tool};
use miyu_tool::{LifeEnding, LifePort, LifeRefused};

use crate::Core;

/// 一个会话的那一份。
pub(crate) struct Ending {
    core: Weak<Core>,
    /// 一次一个：查过「没结束」到记下之间不让第二次插进来。
    one_at_a_time: Mutex<()>,
}

impl Ending {
    /// 给核心 `core` 造一份。
    pub(crate) fn new(core: &Arc<Core>) -> Ending {
        Ending {
            core: Arc::downgrade(core),
            one_at_a_time: Mutex::new(()),
        }
    }

    /// 见模块的说明。
    async fn end_now(
        &self,
        session: &SessionId,
        call: CallId,
        last_words: String,
    ) -> Result<(), LifeRefused> {
        let core = self.core.upgrade().ok_or(LifeRefused::Unavailable)?;
        let _one = self.one_at_a_time.lock().await;
        let unavailable = |reason: &str| {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), reason, "life not ended");
            LifeRefused::Unavailable
        };
        let found = core.sessions.get(&core, session).await;
        let handle = found.map_err(|refusal| unavailable(refusal.reason))?.handle;
        let now = handle.current().await.map_err(|_| unavailable("stopped"))?;
        if !now.life {
            return Err(LifeRefused::NotLife);
        }
        if now.life_ended {
            return Err(LifeRefused::Ended);
        }
        let by = By::Tool(Tool { call_id: call });
        let finishing = super::finish(&core, session, by, Some(last_words))
            .await
            .map_err(|refusal| unavailable(refusal.reason))?;
        let session = session.clone();
        tokio::spawn(async move {
            if let Err(refusal) = finishing.done().await {
                tracing::warn!(target: "miyu::endpoint", session = session.as_str(), reason = refusal.reason, "life not cleared");
            }
        });
        Ok(())
    }
}

impl LifePort for Ending {
    fn end(&self, session: &SessionId, call: &CallId, last_words: String) -> LifeEnding<'_> {
        let (session, call) = (session.clone(), *call);
        Box::pin(async move { self.end_now(&session, call, last_words).await })
    }
}
