//! 嵌套调用的几样纯查询（施工 M-1、M-3，施工 BS-1 中挪出来）：执行器照它们开通道、答 `miyu call --schema`，头重连时接着画。

use super::super::Session;
use super::super::step::Pending;
use super::super::turn::Stage;
use super::{NestedRunning, Phase};
use crate::raw::RawJson;

impl Session {
    /// 脚本里能调的工具：访问类别准、`clicall` 开着的，照名字，带参数格式（施工 M-3）。执行器照它定开不开通道、答
    /// `miyu call --schema`。纯查询。
    pub fn clicall_tools(&self) -> Vec<(String, RawJson)> {
        self.policy
            .tools
            .iter()
            .filter(|(_, rule)| rule.clicall && rule.access.scriptable() && !rule.command_only)
            .map(|(name, rule)| (name.clone(), rule.parameters.clone()))
            .collect()
    }

    /// 有不上工具面、只给核心自带的命令调的工具（施工 BS-1 中）：执行器照它也开通道，会话里的 `miyu pkg` 才连得上。纯查询。
    pub fn has_command_tools(&self) -> bool {
        self.policy.tools.values().any(|rule| rule.command_only)
    }

    /// 在跑的嵌套调用，照编号（施工 M-1，`docs/blueprint/protocol.md` 的订阅）：头断了重连时照它接着画 `#序号 显示名`。纯查询。
    pub fn nested_running(&self) -> Vec<NestedRunning> {
        let mut running: Vec<NestedRunning> = self
            .nested
            .calls
            .iter()
            .filter_map(|call| {
                Some(NestedRunning {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    args: call.args.clone(),
                    at: call.started.filter(|_| call.phase == Phase::Running)?,
                })
            })
            .collect();
        running.sort_by(|a, b| a.id.cmp(&b.id));
        running
    }

    /// 在等人（施工 SL-1，终端第 21 条）：这一步有调用在等确认、在问人，或者命令里脚本调的工具在等确认。会话列表照它写
    /// `waiting`，口径同视图流会话状态的 `state: waiting`。纯查询。
    pub fn waiting_on_person(&self) -> bool {
        let step = match self.turn.as_ref().map(|turn| &turn.stage) {
            Some(Stage::Tools(step)) => step.calls.iter().any(Pending::awaiting_person),
            _ => false,
        };
        step || self
            .nested
            .calls
            .iter()
            .any(|call| call.phase == super::Phase::Asking)
    }
}
