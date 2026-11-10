//! 会话读不了的几种拒绝（施工 YC-2，压测报告 F1 连带）：没有这个会话、日志真读不懂、这一刻读写出错分开说。原来除了「没有」
//! 一律 `session_broken`，打开文件数用光那半小时两千多次载入失败全说成损坏，磁盘上其实一个都没坏。日志中间坏了一行、坏行以前
//! 的还看得到的，载入时另说 `session_read_only`（施工 YC-6，压测报告 F6）。

use miyu_session::LoadError;
use miyu_store::blob::BlobError;
use miyu_store::log::{BrokenAt, OpenError};
use serde_json::json;

use super::{REFUSED, Refusal};

impl Refusal {
    /// 会话这一刻读不了（施工 YC-2）：读日志、策略快照时读写出错（打开文件数用光、权限、磁盘出错），日志本身没坏，可以再试。
    pub(crate) const UNAVAILABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "session_unavailable",
        data: None,
    };

    /// 日志中间坏了一行（施工 YC-6）：坏行以前的照样翻得到（`view.page`），会话载入不了、不能接着聊，要继续新开一个会话。
    /// `data.segment` 是哪一段（只写文件名），`data.line` 是第几行（从这一段的开头数）。日志不动、不修（2026-11-10 项目主人定）。
    pub(crate) fn read_only(at: &BrokenAt) -> Refusal {
        let mut data = serde_json::Map::new();
        data.insert("segment".to_string(), json!(at.segment));
        data.insert("line".to_string(), json!(at.line));
        Refusal {
            code: REFUSED,
            reason: "session_read_only",
            data: Some(data),
        }
    }

    /// 读日志出的错照哪一种拒：没有这个会话的 `session_not_found`，读不懂的 `session_broken`，读写出错的
    /// `session_unavailable`。
    pub(crate) fn of_log(error: &OpenError) -> Refusal {
        match error {
            OpenError::Missing(_) => Refusal::NOT_FOUND,
            OpenError::Broken { .. } => Refusal::BROKEN,
            OpenError::Io(_) => Refusal::UNAVAILABLE,
        }
    }

    /// 载入会话出的错照哪一种拒：日志坏了一行、坏行前面还有的 `session_read_only`（施工 YC-6）；别的日志的照
    /// [`Refusal::of_log`]；策略快照读写出错的 `session_unavailable`；没有造会话那一条、快照没了、对不上、读不懂、造不出策略、过不了
    /// 账本的 `session_broken`；随核心附带的字读不了的是安装坏了，内部出错。
    pub(crate) fn of_load(error: &LoadError) -> Refusal {
        match error {
            LoadError::Log(error) => match error.broken_at().filter(|at| at.seq.get() > 1) {
                Some(at) => Refusal::read_only(&at),
                None => Refusal::of_log(error),
            },
            LoadError::Blob(BlobError::Io(_)) => Refusal::UNAVAILABLE,
            LoadError::Shipped(_) => Refusal::INTERNAL,
            LoadError::Blob(_)
            | LoadError::NotCreated
            | LoadError::Snapshot(_)
            | LoadError::Policy(_)
            | LoadError::Kernel(_) => Refusal::BROKEN,
        }
    }
}

#[cfg(test)]
mod tests;
