//! 人生会话的拒绝（施工 LF-5 中，`docs/blueprint/life.md`「出错」）。

use miyu_kernel::id::SessionId;

use super::{REFUSED, Refusal};

impl Refusal {
    /// 这个人格没有人生会话：`life.pause`、`life.resume` 回它。
    pub(crate) const NO_LIFE: Refusal = Refusal {
        code: REFUSED,
        reason: "no_life",
        data: None,
    };

    /// 这个人格已经有一个没结束的人生会话：`life.start` 回它，`data.session` 是那一个。
    pub(crate) fn life_exists(session: &SessionId) -> Refusal {
        Refusal::with(
            "life_exists",
            "session",
            serde_json::json!(session.as_str()),
        )
    }

    /// 同这一个拒绝，多一格 `data.session`：`life.start` 已经记下身份、定时表那一行没建成时，说清是哪个会话（`life.resume` 照
    /// 日志重建那一行）。
    pub(crate) fn about_session(mut self, session: &SessionId) -> Refusal {
        self.data
            .get_or_insert_with(serde_json::Map::new)
            .insert("session".to_string(), serde_json::json!(session.as_str()));
        self
    }
}
