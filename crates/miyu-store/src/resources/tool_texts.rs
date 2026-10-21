//! 工具结果、拒绝时写给她的几句（施工 O-2 下、X-3 上、O-31 前、5-12；P-9 从 `resources.rs` 拆出来，那边超了行数）：
//! 都不进策略快照，用到的时候读。

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 提供者的工具没在时限里答完（施工 O-2 下，`providers.md`「超时」）：`core/tool-results/timed-out.txt` 的原文，字段 `name`、
    /// `seconds`。不进快照的核心字：进了以后，以前造的会话「核心的字变了」，换不了快照。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn tool_timed_out(&self) -> Result<String, SourceError> {
        self.read(&["core", "tool-results", "timed-out.txt"])
    }

    /// MCP 服务交回的、不给她看的一样东西（施工 X-3 上，`mcp.md`「怎么走」第四条第 3 款：声音、二进制的资源、收不了的图）：
    /// `core/mcp/not-shown.txt` 的原文，字段 `mime`、`bytes`。不进快照，同 [`ResourceRoot::tool_timed_out`]。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn mcp_not_shown(&self) -> Result<String, SourceError> {
        self.read(&["core", "mcp", "not-shown.txt"])
    }

    /// MCP 服务交回的文字太长、后面截掉了（第四条第 4 款）：`core/mcp/cut.txt` 的原文，字段 `cut`（截掉了几个字符）。不进快照。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn mcp_cut(&self) -> Result<String, SourceError> {
        self.read(&["core", "mcp", "cut.txt"])
    }

    /// MCP 服务交的说明放进 system 尾部那一段的外壳（施工 X-39，`mcp.md`「怎么走」第三条第 5 条）：`core/mcp/instructions.txt`
    /// 的原文，字段 `name`（服务名）、`text`（说明的原文）。不进快照：拼好的那一段随 system 记进快照。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn mcp_instructions(&self) -> Result<String, SourceError> {
        self.read(&["core", "mcp", "instructions.txt"])
    }

    /// 权限策略拒绝在场所里做的事（访问类别 `venue`）的那一句：会话不在场所里（施工 O-31 前，`session/guard.md` 第四条）。
    /// 不进策略快照：本机的会话造、载入时读（同 [`ResourceRoot::tool_timed_out`]），进了快照以前造的会话换不了快照。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn not_in_venue(&self) -> Result<String, SourceError> {
        self.read(&["core", "permissions", "not-in-venue.txt"])
    }

    /// 只碰得到自己工作区的会话（外部身份的场所会话，施工 5-12，`11-权限与沙盒.md` 第三节）拒绝时写给她的两句：要碰的路径在
    /// 工作区外面（`outside-workspace.txt`，字段 `path`）、这里还不能跑命令（`no-commands.txt`）。不进策略快照，同
    /// [`ResourceRoot::not_in_venue`]：这种会话造、载入时读。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn confined_texts(&self) -> Result<(String, String), SourceError> {
        Ok((
            self.read(&["core", "permissions", "outside-workspace.txt"])?,
            self.read(&["core", "permissions", "no-commands.txt"])?,
        ))
    }
}
