//! 喂一条事件引起的变化（`docs/blueprint/view.md`「视图流」）：视图流照它推 `view.add`、`view.update`、`view.append`、
//! `view.hidden`、`view.remove`。

use serde::{Deserialize, Serialize};

use crate::entry::{Entry, EntryId};

/// 一样变化。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Change {
    /// 新的一条，排在 `after` 那一条后面（`None` 是最前）。
    Add {
        /// 这一条。
        entry: Entry,
        /// 排在哪一条后面。
        after: Option<EntryId>,
    },
    /// 一条换成这样。
    Update {
        /// 换成的样子。
        entry: Entry,
        /// 位置变了的：排在哪一条后面（里面的 `None` 是最前，写成 `null`）；没变的没有。
        #[serde(
            default,
            deserialize_with = "crate::relay::present",
            skip_serializing_if = "Option::is_none"
        )]
        after: Option<Option<EntryId>>,
    },
    /// 正在写的字接在后面：正文、思考的 `text`，正在写参数的工具的 `args`。
    Append {
        /// 哪一条。
        id: EntryId,
        /// 接上的字。
        text: String,
    },
    /// 这几条藏起来、显示回来：撤销、恢复。
    Hidden {
        /// 哪几条。
        ids: Vec<EntryId>,
        /// 藏起来还是显示回来。
        hidden: bool,
    },
    /// 拿掉一条：流式时开了、落了盘却没有的块（空块、丢掉的半截调用），被打断的压缩。
    Remove {
        /// 哪一条。
        id: EntryId,
    },
}

impl Change {
    /// 视图流在线上的一条推送读回来（施工 V-D 修，终端要的）：`method` 是 `view.add`、`view.update` 这样，`params` 是推送的
    /// 参数（带的 `session` 不管）。
    ///
    /// # Errors
    ///
    /// 不是视图流的推送、参数读不懂的，交回 serde 的错。
    pub fn read(method: &str, mut params: serde_json::Value) -> Result<Change, serde_json::Error> {
        if let (Some(op), Some(fields)) = (method.strip_prefix("view."), params.as_object_mut()) {
            fields.insert("op".to_string(), serde_json::Value::String(op.to_string()));
        }
        serde_json::from_value(params)
    }
}
