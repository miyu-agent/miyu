//! 包在一个会话里的状态（施工 TW-1，设计 `46-包的会话状态.md`）：工具结果顺手带上的（效果 `package.state`），程序直接写的
//! （事件 `package.state`），记成同一种。有效历史照每个包最近一份没撤掉的算（`history/states.rs`）；内核不解读里面的格，
//! 只认空的是「清掉」。

use serde::{Deserialize, Serialize};

use crate::raw::RawJson;

/// `package.state`：包 `package` 在这个会话里换上的一份状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageState {
    /// 哪个包。
    pub package: String,
    /// 换上的那一份：三格都没有的是清掉。
    #[serde(default)]
    pub state: State,
}

/// 一份状态：三格都可以不写，写法由协议端点查（`widgets.md` 的内容、给模型看的字的规矩）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// 界面块：块的编号到 `{"content", "title"?}`，头照包声明的跟着会话的那几块画。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widgets: Option<RawJson>,
    /// 压缩时写进检查点、给模型看的一段英文。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 包自己的数据：内核只存，原样还给包。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<RawJson>,
}

impl State {
    /// 三格都没有：清掉。
    pub fn is_empty(&self) -> bool {
        self.widgets.is_none() && self.note.is_none() && self.data.is_none()
    }

    /// 给头看的那一格：界面块。`note`、`data` 不给头。
    pub fn shown(&self) -> State {
        State {
            widgets: self.widgets.clone(),
            note: None,
            data: None,
        }
    }
}
