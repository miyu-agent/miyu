//! 扩展能力（施工 9-4 下上，`docs/designs/05-内核接口.md` 第三节）：清单的 `[process] capabilities` 声明要哪些，管理员
//! 在开的那一下批（`extensions.md`）。只认这张表里的十五个名字（施工 X-15 上加了 `widgets`，施工 X-20 加了 `context.system`，施工 X-18 加了 `packages.call`），先后照这张表。

/// 一种扩展能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// 向工具目录提供工具。
    Tools,
    /// 提供斜杠命令。
    Commands,
    /// 在界面上显示它自己的内容（施工 X-15 上，设计 `33-界面扩展.md`）。
    Widgets,
    /// 回合开始时往上下文注入内容。
    ContextInject,
    /// 往系统提示词尾部放一段（施工 X-20，`system.append`）。
    ContextSystem,
    /// 工具执行前放行、拒绝，或要求询问人。
    ToolGuard,
    /// 工具执行后改写结果。
    ToolRewrite,
    /// 读取会话的原始事件。
    EventsRead,
    /// 追加自己命名空间下的事件。
    EventsWrite,
    /// 用命令驱动会话。
    SessionsDrive,
    /// 调别的软件包给出来的方法（施工 X-18，`package.call`，对方清单 `[export]` 里的）。
    PackagesCall,
    /// 代表平台上的外部用户发命令：只给通讯平台的桥。
    ActForExternal,
    /// 访问网络。
    Network,
    /// 读指定的路径。
    FsRead,
    /// 写指定的路径。
    FsWrite,
}

impl Capability {
    /// 全部，照表的先后。
    pub const ALL: [Capability; 15] = [
        Capability::Tools,
        Capability::Commands,
        Capability::Widgets,
        Capability::ContextInject,
        Capability::ContextSystem,
        Capability::ToolGuard,
        Capability::ToolRewrite,
        Capability::EventsRead,
        Capability::EventsWrite,
        Capability::SessionsDrive,
        Capability::PackagesCall,
        Capability::ActForExternal,
        Capability::Network,
        Capability::FsRead,
        Capability::FsWrite,
    ];

    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Tools => "tools",
            Capability::Commands => "commands",
            Capability::Widgets => "widgets",
            Capability::ContextInject => "context.inject",
            Capability::ContextSystem => "context.system",
            Capability::ToolGuard => "tool.guard",
            Capability::ToolRewrite => "tool.rewrite",
            Capability::EventsRead => "events.read",
            Capability::EventsWrite => "events.write",
            Capability::SessionsDrive => "sessions.drive",
            Capability::PackagesCall => "packages.call",
            Capability::ActForExternal => "act_for_external",
            Capability::Network => "network",
            Capability::FsRead => "fs.read",
            Capability::FsWrite => "fs.write",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<Capability> {
        Capability::ALL
            .into_iter()
            .find(|capability| capability.as_str() == text)
    }
}
