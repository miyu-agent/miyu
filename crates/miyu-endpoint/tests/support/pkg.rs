//! 会话里的 `miyu pkg`（施工 BS-1 中，设计 `44-基础系统拆包.md` 第三节第 4 条）：`packages`、`package` 不上工具面，她跑的命令经
//! 通道调。测试里换一件假的 `shell`（`Act::Commands`）：她调 `shell` 时写上这条命令里的 `miyu pkg` 调哪几次（[`run`]），
//! 交回的一件一行写成 `<工具> <结局> text=<字>`。别的照出厂的基础系统。

use std::sync::Arc;

use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use super::default_resources;

/// 出厂的基础系统，`shell` 换成假的。
pub fn catalog() -> Catalog {
    let shell = Fake::commanding(
        "shell",
        // 读：假的 `shell` 自己不用问人，问的只有它经通道调的那几次。
        Access::Read,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Commands,
    );
    let mut tools: Vec<Arc<dyn Tool>> = miyu_basesystem::tools(&default_resources())
        .expect("出厂的工具")
        .into_iter()
        .filter(|tool| tool.spec().name != "shell")
        .collect();
    tools.push(shell);
    // 照核心那样一包一组登记（施工 BS-1 上中）：预设照包开关。
    Catalog::in_packages(miyu_basesystem::grouped(tools)).expect("合写法")
}

/// 她调 `shell` 的参数：这条命令里的 `miyu pkg` 照先后调 `commands`（工具名、参数原文）。
pub fn run(commands: &[(&str, &str)]) -> String {
    let commands: Vec<[&str; 2]> = commands.iter().map(|(name, args)| [*name, *args]).collect();
    serde_json::json!({"commands": commands}).to_string()
}

/// 一次调到的跑完了，交回它的字：去掉 `<工具> ok text=` 的开头。不是跑完的照原样。
pub fn text(line: &str) -> &str {
    line.split_once(" ok text=").map_or(line, |(_, text)| text)
}
