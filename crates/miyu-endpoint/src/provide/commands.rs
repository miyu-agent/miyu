//! 扩展的工具声明了 `runs_commands` 的（施工 XG-1 下，设计 `63-扩展工具的效果和声明.md` 第六节第 2 条）：照内置 `shell` 的几处，
//! 一处不按名字。
//!
//! - 说明后面接 `miyu call` 那一段（`Tool::clicall_section`）：字是核心的一份（`core/call/`，`miyu_call::Section`），`shell` 也读它。
//!   例子照这台机器挑：Windows 上 PowerShell 的，别处有 `jq` 用它、没有有 `python3` 用 Python、都没有不带（扩展用哪个 shell 核心
//!   不知道，照内置 `shell` 在这台机器上默认的那一路）。
//! - 通道：会话照现在的条件开端口（`miyu-session` 的 `Tools::channel`，只看 `Tool::runs_commands`），这一次调用拿到了端口、知道
//!   扩展进程的进程号的，开一个认「这个扩展进程的子孙」的端点（[`miyu_call::Kin::descendant_of`]），`tool.call` 多一格
//!   `channel: {sock, token, bin}`。端点跟着这次调用：答了、超时、被掐（future 被丢掉），丢掉就关，`miyu call` 退出码 3。
//!   不知道进程号的（扩展不是核心拉起的，测试里的假连接）不开、不带这一格，命令里的 `miyu call` 照「不在 Miyu 跑的命令里」说。

use std::sync::Arc;

use miyu_call::{Channel, Example, Handed, Kin, Section, Texts};
use miyu_tool::Call;

use crate::Core;
use crate::refusal::Refusal;

/// 跑命令的工具共用的两样：说明末尾那一段、通道答回去的几句。一个包登记一次读一份。
#[derive(Clone)]
pub(super) struct Commands {
    section: Arc<Section>,
    texts: Arc<Texts>,
}

impl Commands {
    /// 照核心的资源目录读：读不出来的（出厂的字坏了）`internal`，记一行。
    pub(super) fn load(core: &Core) -> Result<Commands, Refusal> {
        let resources = core.resources.path();
        let example = Example::pick(!cfg!(windows), std::env::var_os("PATH").as_deref());
        let loaded = Section::load(resources, example)
            .and_then(|section| Ok((section, Texts::load(resources)?)));
        match loaded {
            Ok((section, texts)) => Ok(Commands {
                section: Arc::new(section),
                texts,
            }),
            Err(error) => {
                tracing::error!(target: "miyu::endpoint", error = %error.why, "miyu call texts not loaded");
                Err(Refusal::INTERNAL)
            }
        }
    }

    /// 接在说明后面的那一段：`tools` 是能调的名单，`shapes` 是结果的样子。
    pub(super) fn section(&self, tools: &str, shapes: &str) -> String {
        self.section.render(tools, shapes)
    }

    /// 这一次调用 `call` 的端点：拿到了端口、知道扩展进程 `pid` 的才开。交回端点（丢掉就关）和交给扩展的那一格；开不成的记一行，
    /// 调用照旧发，只是没有这一格。
    pub(super) fn open(
        &self,
        call: &Call,
        package: &str,
        pid: Option<u32>,
    ) -> Option<(Channel, Handed)> {
        let port = call.nested.clone()?;
        let Some(pid) = pid else {
            tracing::debug!(target: "miyu::endpoint", package, "miyu call endpoint not opened: no extension process");
            return None;
        };
        let channel = Channel::open(port, Arc::clone(&self.texts), Kin::descendant_of(pid))
            .inspect_err(|error| {
                tracing::warn!(target: "miyu::endpoint", package, error = %error, "miyu call endpoint not opened");
            })
            .ok()?;
        let handed = channel.handed()?;
        Some((channel, handed))
    }
}
