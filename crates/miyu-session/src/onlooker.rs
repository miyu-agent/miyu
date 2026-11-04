//! 落了盘的事件顺手交给旁边看着的（施工 V-44，`docs/construction/V-44-判官记录窗.md`）：端点照它给载入着的群会话维护判官的
//! 记录窗（`miyu-endpoint` 的 `venues/records/recent.rs`）。一个会话至多一个，第一次要的时候装上（[`crate::Handle::onlooker`]），
//! 和 actor 共用；会话退下、停了跟着放掉。没装的会话 actor 什么都不多做。

use std::sync::{Arc, OnceLock};

use miyu_kernel::event::Event;

/// 看着一个会话落盘的：actor 每写进去一批（照先后、序号接着），写完、回应之前交给它。在 actor 里同步调，只该做很快的事。
pub trait Onlooker: std::any::Any + Send + Sync + std::fmt::Debug {
    /// 写进去了一批。
    fn appended(&self, events: &[Event]);
}

/// actor 和把手共用的那一格：装上以后不换。
pub(crate) type Onlookers = Arc<OnceLock<Arc<dyn Onlooker>>>;
