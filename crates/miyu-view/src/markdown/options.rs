//! 三个预设和盖过预设的开关（调研第五节第 2、3 条，施工 SE-1 中下中）：预设只是几个开关的默认值，协议里写了的开关盖过它。
//!
//! | 预设 | 范围 | 思考 | 工具 |
//! |---|---|---|---|
//! | `read` 自己看 | 全部 | 带 | 她看到的（显示名、摘要，长的折起来） |
//! | `agent` 给别的 agent | 她现在的上下文 | 不带 | 她看到的（真名、参数原文，结果截到头尾约 4000 字） |
//! | `chat` 贴网页 chat | 全部 | 不带 | 一行 |
//!
//! `children` 这一步只有 `reports`（交代和回报照预设的样子写）、`assets` 只有 `placeholder`（写占位），渲染器里不另设开关。

/// 给谁用：定写法（小标题、工具怎么写、开头），开关的默认值照它。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// 自己看：显示名、旁白跟语言。
    Read,
    /// 给别的 agent：开头一段英文事实，工具写真名和参数原文。
    Agent,
    /// 贴进网页 chat：工具写真名，一行写完，开头写 token 估算。
    Chat,
}

/// 工具写多少。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tools {
    /// 一行：做了什么、成没成，出错的带上错误。
    Line,
    /// 她当时看到的：`agent`、`chat` 的写法另把每条结果截到头尾共约 4000 字。
    Seen,
    /// 全文：日志里她看到的整份不再截；后台命令的输出、脚本里调的工具交回的结果去 blob 取（[`super::Sources::blobs`]）。
    Full,
}

/// 导多少。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// 界面上看得到的全部。
    All,
    /// 她现在的上下文：从最近一次还算数的检查点起（同请求组装的有效历史，[`super::Sources::settle`]）。
    Context,
    /// 最近几轮（至少 1）。
    Turns(u32),
}

/// 一套开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// 给谁用。
    pub purpose: Purpose,
    /// 带思考。
    pub thinking: bool,
    /// 工具写多少。
    pub tools: Tools,
    /// 导多少。
    pub range: Range,
}

impl Options {
    /// `read` 预设：自己看。
    pub const READ: Options = Options {
        purpose: Purpose::Read,
        thinking: true,
        tools: Tools::Seen,
        range: Range::All,
    };

    /// `agent` 预设：给别的 agent。
    pub const AGENT: Options = Options {
        purpose: Purpose::Agent,
        thinking: false,
        tools: Tools::Seen,
        range: Range::Context,
    };

    /// `chat` 预设：贴进网页 chat。
    pub const CHAT: Options = Options {
        purpose: Purpose::Chat,
        thinking: false,
        tools: Tools::Line,
        range: Range::All,
    };

    /// 照 `purpose` 的预设。
    #[must_use]
    pub fn preset(purpose: Purpose) -> Options {
        match purpose {
            Purpose::Read => Options::READ,
            Purpose::Agent => Options::AGENT,
            Purpose::Chat => Options::CHAT,
        }
    }
}
