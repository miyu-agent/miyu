//! 空闲的会话退下，把手这一头（施工 V-2 再补，`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条，
//! `docs/blueprint/session/actor.md`「退下」）：会话表拿着表的锁问，actor 照自己的账答（`actor/life.rs`）。
//!
//! 封存了的会话退得下了告诉会话表（施工 V-51，[`Bell`]）：会话表当场来问，不等隔一阵问的那一遍（最多 3 分钟）。
//!
//! 内存紧时会话表照 [`Retiring::Tight`] 问（施工 MEM-3，`docs/blueprint/protocol.md`「会话表」第 10 条）：不等闲够，排着、还没
//! 开始的事（记忆抽取的闹钟）不算还有事；还有事的答是哪一样（[`Hold`]），会话表照它分原因记日志。

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use miyu_kernel::id::SessionId;

use super::{Handle, Message, Stopped};
use crate::SessionPort;
use crate::agents::Agents;

/// 问「闲够了没有」的回答。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retire {
    /// 退下了：actor 不再收信，会话表拿掉它，下次用到再载入。
    Retired,
    /// 别的都空着，只是闲得还不够久：还差这么久。
    Later(Duration),
    /// 还有事：回合、头看着、派出去还没回报的、记忆的闹钟、有会话等它空下来……过一阵再问。带着是哪一样（施工 MEM-3）。
    Kept(Hold),
}

/// 会话表怎么问（施工 MEM-3）。`Duration` 转成 [`Retiring::Idle`]：闲够了退下的那一路照旧写 `retire(idle)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retiring {
    /// 闲够这么久了没有（`protocol.md`「会话表」第 9 条）：还差的答 [`Retire::Later`]，闹钟上着的算还有事。
    Idle(Duration),
    /// 内存紧（「会话表」第 10 条）：没事就退，不等闲够。记忆抽取的闹钟上着、还没响的不算还有事：退下就和核心退出一样，
    /// 这一段等会话下次载入、闲下来再抽（`memory.md` 第六条第 1 款）；在路上的、不等闲的那一次在等着的照旧算。
    Tight,
}

impl From<Duration> for Retiring {
    fn from(idle: Duration) -> Retiring {
        Retiring::Idle(idle)
    }
}

/// 还有事的是哪一样（施工 MEM-3）：内存紧时卸不动的，会话表照它分原因计数、记日志。actor 照 [`Hold::ALL`] 的先后查，交头一个
/// 碰上的；前三样会话表自己就看得出，不问 actor（`Handle::busy`、`Handle::watched`、`Handle::alone`），actor 那边也照查。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hold {
    /// 内核不空：有回合在跑、在重启、在等子代理回报。
    Turn,
    /// 有头订阅着。
    Watched,
    /// 除了会话表那一个还有别的把手：正在办的请求拿着（只有会话表那一头看）。
    Handles,
    /// 有等着回应的命令。
    Replies,
    /// 有请求在路上：主请求、起标题、回顾。
    Requests,
    /// 派出去的任务还没回报，或者结束了还没落盘。
    Jobs,
    /// 记忆抽取：在路上、不等闲的那一次在等着；闲够了退下的那一路闹钟上着也算。
    Memory,
    /// 会话之间等着：有会话等它空下来，或者它在等别的会话。
    Peers,
    /// 收件箱、回报的路里还有没办的。
    Mail,
    /// 长期目标还要接着续轮。
    Goal,
    /// 1 秒内没答上：手里那一封办得慢，当它还有事。
    Slow,
}

impl Hold {
    /// 全部，照 actor 查的先后。
    pub const ALL: [Hold; 11] = [
        Hold::Turn,
        Hold::Watched,
        Hold::Handles,
        Hold::Replies,
        Hold::Requests,
        Hold::Jobs,
        Hold::Memory,
        Hold::Peers,
        Hold::Mail,
        Hold::Goal,
        Hold::Slow,
    ];

    /// 记日志用的名字：小写英文一个词。
    pub fn name(self) -> &'static str {
        match self {
            Hold::Turn => "turn",
            Hold::Watched => "watched",
            Hold::Handles => "handles",
            Hold::Replies => "replies",
            Hold::Requests => "requests",
            Hold::Jobs => "jobs",
            Hold::Memory => "memory",
            Hold::Peers => "peers",
            Hold::Mail => "mail",
            Hold::Goal => "goal",
            Hold::Slow => "slow",
        }
    }
}

/// 最多等 actor 答多久：它手里那一封办得慢的，这一次当它还有事，会话表不一直拿着锁等。
const ANSWER_WAIT: Duration = Duration::from_secs(1);

impl Handle {
    /// 除了这一个，没有别的把手（施工 V-2 再补）：正在办的请求、订阅着的头都拿着一个。会话表拿着表的锁看它：这一刻谁也
    /// 拿不到新的把手，问退下时没有第二个写者的可能。
    pub fn alone(&self) -> bool {
        self.inbox.strong_count() == 1
    }

    /// 闲了 `idle` 以上、什么事都没有的退下（施工 V-2 再补）；照 [`Retiring::Tight`] 问的没事就退（施工 MEM-3）。答得慢的叫它
    /// 别答了，当还有事（[`Hold::Slow`]）；那一刻已经答了的照答的算。回不进来的 actor 不退：会话表还当它在跑。
    ///
    /// # Errors
    ///
    /// 会话已经停了。
    pub async fn retire(&self, ask: impl Into<Retiring>) -> Result<Retire, Stopped> {
        let (reply, mut answer) = tokio::sync::oneshot::channel();
        self.send(Message::Retire {
            ask: ask.into(),
            reply,
        })?;
        match tokio::time::timeout(ANSWER_WAIT, &mut answer).await {
            Ok(answered) => answered.map_err(|_| Stopped),
            Err(_) => {
                answer.close();
                Ok(answer.try_recv().unwrap_or(Retire::Kept(Hold::Slow)))
            }
        }
    }
}

/// 封存了的会话退得下了没有（施工 V-51）：actor 和每一个把手共用一份旗。actor 每办完一封看一次自己的账（`life.rs`），封存了、
/// 没事了就立起来、告诉会话表（[`SessionPort::retirable`]）；会话表问过就放下。旗立着时放下一个把手也告诉一次：会话表上次来问
/// 时它还拿着，那一次「没有别的把手」不成。没有会话表的端口的（测试里自己造的会话）不告诉。
pub(crate) struct Bell {
    ready: AtomicBool,
    table: Option<(Arc<dyn SessionPort>, SessionId)>,
}

impl Bell {
    /// 照这个会话派子代理用的那一份端口告诉会话表（会话表造会话、载入时交进来）。
    pub(crate) fn new(agents: Option<&Arc<Agents>>) -> Arc<Bell> {
        let table = agents.map(|agents| (Arc::clone(&agents.port), agents.session.clone()));
        Arc::new(Bell {
            ready: AtomicBool::new(false),
            table,
        })
    }

    fn ring(&self) {
        if let Some((port, session)) = &self.table {
            port.retirable(session.clone());
        }
    }

    /// actor 看了一遍：`ready` 是这时退不退得下。刚立起来的告诉会话表。
    pub(crate) fn offer(&self, ready: bool) {
        if ready && !self.ready.swap(true, Ordering::AcqRel) {
            self.ring();
        } else if !ready {
            self.ready.store(false, Ordering::Release);
        }
    }

    /// 会话表来问了：放下。没退的，actor 下一次看时还退得下就再立起来、再告诉一次。
    pub(crate) fn lower(&self) {
        self.ready.store(false, Ordering::Release);
    }
}

/// 把手拿着的那一份旗：每一份放下时，旗立着的告诉会话表。
#[derive(Clone)]
pub(crate) struct Leaving(Arc<Bell>);

impl Leaving {
    /// 和 actor 共用 `bell`。
    pub(crate) fn of(bell: &Arc<Bell>) -> Leaving {
        Leaving(Arc::clone(bell))
    }

    /// 不告诉谁的（测试里直接造的把手）。
    #[cfg(test)]
    pub(crate) fn none() -> Leaving {
        Leaving(Bell::new(None))
    }
}

impl Drop for Leaving {
    fn drop(&mut self) {
        if self.0.ready.load(Ordering::Acquire) {
            self.0.ring();
        }
    }
}

impl fmt::Debug for Leaving {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Leaving")
            .field(&self.0.ready.load(Ordering::Relaxed))
            .finish()
    }
}
